use crate::clauses::{Clause, ClauseDB, ClauseId, ClausesParams, Watches};
use crate::solver::{Binding, BindingResult, EnforceResult};
use aries_backtrack::Backtrack;
use aries_backtrack::{QReader, Q};
use aries_model::assignments::Assignment;
use aries_model::int_model::{Cause, DiscreteModel, DomEvent, EmptyDomain, ILit, VarEvent};
use aries_model::lang::*;
use aries_model::{Model, WModel, WriterId};
use smallvec::alloc::collections::VecDeque;
use std::num::NonZeroU32;

type BoolChanges = QReader<(VarEvent, Cause)>;

pub struct SatSolver {
    clauses: ClauseDB,
    watches: Watches,
    events_stream: QReader<(VarEvent, Cause)>,
    token: WriterId,
    /// Clauses that have been added to the database but not processed and propagated yet
    pending_clauses: VecDeque<ClauseId>,
}
impl SatSolver {
    pub fn new(token: WriterId, model: &mut Model) -> SatSolver {
        SatSolver {
            clauses: ClauseDB::new(ClausesParams::default()),
            watches: Watches::default(),
            events_stream: model.event_stream(),
            token,
            pending_clauses: Default::default(),
        }
    }

    /// Adds a new clause that will be part of the problem definition.
    /// Returns a unique and stable identifier for the clause.
    pub fn add_clause(&mut self, clause: &[ILit]) -> ClauseId {
        self.add_clause_impl(clause, false)
    }

    /// Adds a clause that is implied by the other clauses and that the solver is allowed to forget if
    /// it judges that its constraint database is bloated and that this clause is not helpful in resolution.
    pub fn add_forgettable_clause(&mut self, clause: &[ILit]) {
        self.add_clause_impl(clause, true);
    }

    fn add_clause_impl(&mut self, clause: &[ILit], learnt: bool) -> ClauseId {
        let cl_id = self.clauses.add_clause(Clause::new(&clause, learnt), true);
        self.pending_clauses.push_back(cl_id);
        cl_id
    }

    /// Process a newly added clause, making no assumption on the status of the clause.
    ///
    /// The only requirement is that the clause should not have been processed yet.
    fn process_arbitrary_clause(&mut self, cl_id: ClauseId, model: &mut WModel) -> Option<ClauseId> {
        let clause = self.clauses[cl_id].disjuncts.as_slice();
        if clause.is_empty() {
            // empty clause is always conflicting
            return Some(cl_id);
        } else if clause.len() == 1 {
            let l = clause[0];
            self.watches.add_watch(cl_id, !l);
            return match model.value_of_literal(l) {
                None => {
                    model.set(l, cl_id);
                    None
                }
                Some(true) => None,
                Some(false) => Some(cl_id),
            };
        }
        debug_assert!(clause.len() >= 2);

        // clause has at least two literals
        self.clauses[cl_id].move_watches_front(
            |l| model.value_of_literal(l),
            |l| model.view().implying_event(&l).unwrap().decision_level,
        );
        let clause = &self.clauses[cl_id].disjuncts;
        let l0 = clause[0];
        let l1 = clause[1];

        if model.entails(l0) {
            // satisfied, set watchers and leave state unchanged
            self.set_watch_on_first_literals(cl_id);
            None
        } else if model.entails(!l0) {
            // violated
            debug_assert!(model.violated_clause(&clause));
            self.set_watch_on_first_literals(cl_id);
            Some(cl_id)
        } else if model.value_of_literal(l1).is_none() {
            // pending, set watch and leave state unchanged
            debug_assert!(model.is_undefined_literal(l0));
            debug_assert!(model.pending_clause(&clause));
            self.set_watch_on_first_literals(cl_id);
            None
        } else {
            // clause is unit
            debug_assert!(model.is_undefined_literal(l0));
            debug_assert!(model.unit_clause(&clause));
            self.process_unit_clause(cl_id, model);
            None
        }
    }

    fn process_unit_clause(&mut self, cl_id: ClauseId, model: &mut WModel) {
        let clause = &self.clauses[cl_id].disjuncts;
        debug_assert!(model.unit_clause(clause));

        if clause.len() == 1 {
            let l = clause[0];
            debug_assert!(model.is_undefined_literal(l));
            debug_assert!(!self.watches.is_watched_by(!l, cl_id));
            // watch the only literal
            self.watches.add_watch(cl_id, !l);
            model.set(l, cl_id).unwrap();
        } else {
            debug_assert!(clause.len() >= 2);

            // Set up watch, the first literal must be undefined and the others violated.
            self.clauses[cl_id].move_watches_front(
                |l| model.value_of_literal(l),
                |l| model.view().implying_event(&l).unwrap().decision_level,
            );

            let l = self.clauses[cl_id].disjuncts[0];
            debug_assert!(model.is_undefined_literal(l));
            debug_assert!(model.violated_clause(&self.clauses[cl_id].disjuncts[1..]));
            self.set_watch_on_first_literals(cl_id);
            model.set(l, cl_id).unwrap();
        }
    }

    pub fn propagate(&mut self, model: &mut WModel) -> Result<(), ClauseId> {
        // process all clauses that have been added since last propagation
        while let Some(cl) = self.pending_clauses.pop_front() {
            if let Some(conflict) = self.process_arbitrary_clause(cl, model) {
                return Err(conflict);
            }
        }
        // grow or shrink database. Placed here to be as close as possible to initial minisat
        // implementation where this appeared in search
        // self.scale_database(); TODO: place somewhere

        self.propagate_enqueued(model)
    }

    /// Returns:
    ///   Err(i): in case of a conflict where i is the id of the violated clause
    ///   Ok(()) if no conflict was detected during propagation
    fn propagate_enqueued(&mut self, model: &mut WModel) -> Result<(), ClauseId> {
        debug_assert!(
            self.pending_clauses.is_empty(),
            "Some clauses have not been integrated in the database yet."
        );

        while let Some((ev, _)) = self.events_stream.pop() {
            println!("Propagating event: {:?}", ev);
            let var = ev.var;
            let new_lit = ILit::from(ev);
            match ev.ev {
                DomEvent::NewUB { prev, new } => {
                    let mut watches = Vec::new();
                    self.watches.move_ub_watches_to(var, &mut watches); // TODO: is this really lb
                    println!("Watches: {:?}", &watches);
                    for i in 0..watches.len() {
                        let ub_watch = &watches[i];
                        let watched_lit = ub_watch.to_lit(var);
                        if new_lit.entails(watched_lit) {
                            if !self.propagate_clause(ub_watch.watcher, new_lit, model) {
                                // clause violated, restore remaining watches
                                for watch in &watches[i + 1..] {
                                    self.watches.add_watch(watch.watcher, watch.to_lit(var));
                                }
                                return Err(ub_watch.watcher);
                            }
                        } else {
                            // not implied
                            self.watches.add_watch(ub_watch.watcher, watched_lit)
                        }
                    }
                }
                DomEvent::NewLB { prev, new } => {
                    let mut watches = Vec::new();
                    self.watches.move_lb_watches_to(var, &mut watches);
                    println!("Watches: {:?}", &watches);
                    for i in 0..watches.len() {
                        let lb_watch = &watches[i];
                        let watched_lit = lb_watch.to_lit(var);
                        if new_lit.entails(watched_lit) {
                            if !self.propagate_clause(lb_watch.watcher, new_lit, model) {
                                // clause violated, restore remaining watches
                                for watch in &watches[i + 1..] {
                                    self.watches.add_watch(watch.watcher, watch.to_lit(var));
                                }
                                return Err(lb_watch.watcher);
                            }
                        } else {
                            // not implied
                            self.watches.add_watch(lb_watch.watcher, watched_lit)
                        }
                    }
                }
            }
            //     self.propagation_work_buffer.clear();
            //     for x in self.watches[p].drain(..) {
            //         self.propagation_work_buffer.push(x);
            //     }
            //
            //     let n = self.propagation_work_buffer.len();
            //     for i in 0..n {
            //         if !self.propagate_clause(self.propagation_work_buffer[i], p) {
            //             // clause violated
            //             // restore remaining watches
            //             for j in i + 1..n {
            //                 self.watches[p].push(self.propagation_work_buffer[j]);
            //             }
            //             self.propagation_queue.clear();
            //             self.check_invariants();
            //             self.search_state.status = SearchStatus::Conflict;
            //             return Some(self.propagation_work_buffer[i]);
            //         }
            //     }
        }
        Ok(())
    }

    /// Propagate a clause that is watching literal `p` became true.
    /// `p` should be one of the literals watched by the clause.
    /// If the clause is:
    /// - pending: reset another watch and return true
    /// - unit: reset watch, enqueue the implied literal and return true
    /// - violated: reset watch and return false
    fn propagate_clause(&mut self, clause_id: ClauseId, p: ILit, model: &mut WModel) -> bool {
        debug_assert_eq!(model.value_of_literal(p), Some(true));
        // counter intuitive: this method is only called after removing the watch
        // and we are responsible for resetting a valid watch.
        debug_assert!(!self.watches.is_watched_by(p, clause_id));
        // self.stats.propagations += 1;
        let lits = &mut self.clauses[clause_id].disjuncts;
        if lits.len() == 1 {
            debug_assert!(p.entails(!lits[0]));
            // only one literal that is false, the clause is in conflict
            self.watches.add_watch(clause_id, p);
            return false;
        }
        if p.entails(!lits[0]) {
            lits.swap(0, 1);
        }
        debug_assert!(p.entails(!lits[1])); // lits[1] == !p in SAT
        debug_assert!(model.value_of_literal(lits[1]) == Some(false));
        let lits = &self.clauses[clause_id].disjuncts;
        if model.entails(lits[0]) {
            // clause satisfied, restore the watch and exit
            self.watches.add_watch(clause_id, !lits[1]);
            return true;
        }
        // look for replacement for lits[1] : a literal that is not false.else
        // we look for them in the unwatched literals (i.e. all but the first two ones)
        for i in 2..lits.len() {
            if !model.entails(!lits[i]) {
                let lits = &mut self.clauses[clause_id].disjuncts;
                lits.swap(1, i);
                self.watches.add_watch(clause_id, !lits[1]);
                return true;
            }
        }
        // no replacement found, clause is unit
        self.watches.add_watch(clause_id, !lits[1]);
        let first_lit = lits[0];
        model.set(first_lit, clause_id).is_ok()
    }

    /// set the watch on the first two literals of the clause (without any check)
    /// One should typically call `move_watches_front` on the clause before hand.
    fn set_watch_on_first_literals(&mut self, cl_id: ClauseId) {
        let cl = &self.clauses[cl_id].disjuncts;
        debug_assert!(cl.len() >= 2);
        self.watches.add_watch(cl_id, !cl[0]);
        self.watches.add_watch(cl_id, !cl[1]);
    }

    fn assert_watches_valid(&self, cl_id: ClauseId, model: &Model) -> bool {
        let cl = self.clauses[cl_id].disjuncts.as_slice();
        let l0 = cl[0];
        let l1 = cl[1];
        // assert!(self.watches[!l0].contains(&cl_id));
        // assert!(self.watches[!l1].contains(&cl_id));
        match model.discrete.or_value(cl) {
            Some(true) => {
                // one of the two watches should be entailed
                assert!(model.discrete.entails(&l0) || model.discrete.entails(&l1))
            }
            Some(false) => {}
            None => {
                // both watches should be undefined. If only one was undef, then the clause should have replaced the other watch
                // it with an undefined literal, or do unit propagation which should have made the clause true
                assert!(model.discrete.value(&l0).is_none() && model.discrete.value(&l1).is_none())
            }
        }
        true
    }

    pub fn bind(
        &mut self,
        reif: ILit,
        e: &Expr,
        bindings: &mut Q<Binding>,
        model: &mut DiscreteModel,
    ) -> BindingResult {
        // match e.fun {
        //     Fun::Or => {
        //         let mut disjuncts = Vec::with_capacity(e.args.len());
        //         for &a in &e.args {
        //             let a = BAtom::try_from(a).expect("not a boolean");
        //             let lit = self.reify(a, model);
        //             bindings.push(Binding::new(lit, a));
        //             disjuncts.push(lit);
        //         }
        //         let mut clause = Vec::with_capacity(disjuncts.len() + 1);
        //         // make reif => disjuncts
        //         clause.push(!reif);
        //         disjuncts.iter().for_each(|l| clause.push(*l));
        //         self.sat.add_clause(&clause);
        //         for disjunct in disjuncts {
        //             // enforce disjunct => reif
        //             clause.clear();
        //             clause.push(!disjunct);
        //             clause.push(reif);
        //             self.sat.add_clause(&clause);
        //         }
        //         BindingResult::Refined
        //     }
        //     _ => BindingResult::Unsupported,
        // }
        todo!()
    }

    fn tautology(&mut self) -> ILit {
        // if let Some(tauto) = self.tautology {
        //     tauto
        // } else {
        //     let tauto = self.sat.add_var().true_lit();
        //     self.tautology = Some(tauto);
        //     self.sat.add_clause(&[tauto]);
        //     tauto
        // }
        todo!()
    }

    pub fn enforce(&mut self, b: BAtom, i: &mut Model, bindings: &mut Q<Binding>) -> EnforceResult {
        // // force literal to be true
        // // TODO: we should check if the variable already exists and if not, provide tautology instead
        // let lit = self.reify(b, &mut i.discrete);
        // self.sat.add_clause(&[lit]);
        //
        // if let BAtom::Expr(b) = b {
        //     match i.expressions.expr_of(b) {
        //         NExpr::Pos(e) => match e.fun {
        //             Fun::Or => {
        //                 let mut lits = Vec::with_capacity(e.args.len());
        //                 for &a in &e.args {
        //                     let a = BAtom::try_from(a).expect("not a boolean");
        //                     let lit = self.reify(a, &mut i.discrete);
        //                     bindings.push(Binding::new(lit, a));
        //                     lits.push(lit);
        //                 }
        //                 self.sat.add_clause(&lits);
        //
        //                 EnforceResult::Refined
        //             }
        //             _ => EnforceResult::Reified(lit),
        //         },
        //         NExpr::Neg(e) => match e.fun {
        //             Fun::Or => {
        //                 // a negated OR, treat it as and AND
        //                 for &a in &e.args {
        //                     let a = BAtom::try_from(a).expect("not a boolean");
        //                     let lit = self.reify(a, &mut i.discrete);
        //                     bindings.push(Binding::new(lit, a));
        //                     self.sat.add_clause(&[!lit]);
        //                 }
        //
        //                 EnforceResult::Refined
        //             }
        //             _ => EnforceResult::Reified(lit),
        //         },
        //     }
        // } else {
        //     // Var or constant, enforce at beginning
        //     EnforceResult::Enforced
        // }
        todo!()
    }

    fn reify(&mut self, b: BAtom, model: &mut DiscreteModel) -> ILit {
        // match b {
        //     BAtom::Cst(true) => self.tautology(),
        //     BAtom::Cst(false) => !self.tautology(),
        //     BAtom::Var { var, negated } => {
        //         let lit = model.intern_variable_with(var, || self.sat.add_var().true_lit());
        //         if negated {
        //             !lit
        //         } else {
        //             lit
        //         }
        //     }
        //     BAtom::Expr(e) => {
        //         let BExpr { expr, negated } = e;
        //         let lit = model.intern_expr_with(expr, || self.sat.add_var().true_lit());
        //         if negated {
        //             !lit
        //         } else {
        //             lit
        //         }
        //     }
        // }
        todo!()
    }

    // pub fn propagate(
    //     &mut self,
    //     model: &mut DiscreteModel,
    //     on_learnt_clause: impl FnMut(&[ILit]),
    // ) -> SatPropagationResult {
    //     // // process pending model events
    //     // while let Some((lit, cause)) = self.changes.pop() {
    //     //     match cause {
    //     //         Cause::Decision => panic!(),
    //     //         Cause::Inference(InferenceCause { writer, payload: _ }) => {
    //     //             if writer != self.token {
    //     //                 self.sat.assume(lit);
    //     //             } else {
    //     //                 debug_assert_eq!(
    //     //                     self.sat.get_literal(lit),
    //     //                     Some(true),
    //     //                     "We set a literal ourselves, but the solver does know aboud id"
    //     //                 );
    //     //             }
    //     //         }
    //     //     }
    //     // }
    //     // match self.sat.propagate() {
    //     //     PropagationResult::Conflict(clause) => {
    //     //         // we must handle conflict and backtrack in theory
    //     //         match self.sat.handle_conflict(clause, on_learnt_clause) {
    //     //             ConflictHandlingResult::Backtracked {
    //     //                 num_backtracks,
    //     //                 inferred,
    //     //             } => {
    //     //                 model.restore(model.num_saved() - num_backtracks.get());
    //     //                 model.set(inferred, self.token.cause(0u64));
    //     //                 SatPropagationResult::Backtracked(num_backtracks)
    //     //             }
    //     //             ConflictHandlingResult::Unsat => SatPropagationResult::Unsat,
    //     //         }
    //     //     }
    //     //     PropagationResult::Inferred(lits) => {
    //     //         if lits.is_empty() {
    //     //             SatPropagationResult::NoOp
    //     //         } else {
    //     //             for l in lits {
    //     //                 model.set(*l, self.token.cause(0u64));
    //     //             }
    //     //             SatPropagationResult::Inferred
    //     //         }
    //     //     }
    //     // }
    //     todo!()
    // }
}

impl Backtrack for SatSolver {
    fn save_state(&mut self) -> u32 {
        todo!()
        // self.sat.save_state().num_decisions() - 1
    }

    fn num_saved(&self) -> u32 {
        // self.sat.decision_level().num_decisions()
        todo!()
    }

    fn restore_last(&mut self) {
        todo!()
        // if !self.sat.backtrack() {
        //     panic!("No state to restore.");
        // }
    }
}

#[cfg(test)]
mod tests {
    use crate::solver::sat_solver::SatSolver;
    use aries_backtrack::Backtrack;
    use aries_model::assignments::Assignment;
    use aries_model::int_model::{Cause, ILit, IntDomain};
    use aries_model::lang::IntCst;
    use aries_model::{Model, WriterId};

    #[test]
    fn test_propagation_simple() {
        let writer = WriterId::new(1u8);
        let mut model = Model::new();
        let a = model.new_bvar("a");
        let b = model.new_bvar("b");

        let mut sat = SatSolver::new(writer, &mut model);
        let a_or_b = vec![ILit::geq(a, 1), ILit::geq(b, 1)];

        sat.add_clause(&a_or_b);
        sat.propagate(&mut model.writer(writer)).unwrap();
        assert_eq!(model.boolean_value_of(a), None);
        assert_eq!(model.boolean_value_of(b), None);
        model.discrete.set_ub(a, 0, Cause::Decision).unwrap();
        assert_eq!(model.boolean_value_of(a), Some(false));
        assert_eq!(model.boolean_value_of(b), None);
        sat.propagate(&mut model.writer(writer)).unwrap();
        assert_eq!(model.boolean_value_of(a), Some(false));
        assert_eq!(model.boolean_value_of(b), Some(true));
    }

    #[test]
    fn test_propagation_complex() {
        let writer = WriterId::new(1u8);
        let mut model = Model::new();
        let a = model.new_bvar("a");
        let b = model.new_bvar("b");
        let c = model.new_bvar("c");
        let d = model.new_bvar("d");

        let check_values = |model: &Model, values: [Option<bool>; 4]| {
            assert_eq!(model.boolean_value_of(a), values[0]);
            assert_eq!(model.boolean_value_of(b), values[1]);
            assert_eq!(model.boolean_value_of(c), values[2]);
            assert_eq!(model.boolean_value_of(d), values[3]);
        };
        check_values(&model, [None, None, None, None]);

        let mut sat = SatSolver::new(writer, &mut model);
        let clause = vec![a.true_lit(), b.true_lit(), c.true_lit(), d.true_lit()];

        sat.add_clause(&clause);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [None, None, None, None]);

        model.save_state();
        model.discrete.decide(a.false_lit()).unwrap();
        check_values(&model, [Some(false), None, None, None]);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [Some(false), None, None, None]);

        model.save_state();
        model.discrete.decide(b.false_lit()).unwrap();
        check_values(&model, [Some(false), Some(false), None, None]);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [Some(false), Some(false), None, None]);

        model.save_state();
        model.discrete.decide(c.true_lit()).unwrap();
        check_values(&model, [Some(false), Some(false), Some(true), None]);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [Some(false), Some(false), Some(true), None]);

        model.save_state();
        model.discrete.decide(d.false_lit()).unwrap();
        check_values(&model, [Some(false), Some(false), Some(true), Some(false)]);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [Some(false), Some(false), Some(true), Some(false)]);

        model.restore_last();
        check_values(&model, [Some(false), Some(false), Some(true), None]);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [Some(false), Some(false), Some(true), None]);

        model.restore_last();
        check_values(&model, [Some(false), Some(false), None, None]);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [Some(false), Some(false), None, None]);

        model.discrete.decide(c.false_lit()).unwrap();
        check_values(&model, [Some(false), Some(false), Some(false), None]);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [Some(false), Some(false), Some(false), Some(true)]);
    }

    #[test]
    fn test_propagation_failure() {
        let writer = WriterId::new(1u8);
        let mut model = Model::new();
        let a = model.new_bvar("a");
        let b = model.new_bvar("b");

        let mut sat = SatSolver::new(writer, &mut model);
        let a_or_b = vec![ILit::geq(a, 1), ILit::geq(b, 1)];

        sat.add_clause(&a_or_b);
        sat.propagate(&mut model.writer(writer)).unwrap();
        assert_eq!(model.boolean_value_of(a), None);
        assert_eq!(model.boolean_value_of(b), None);
        model.discrete.set_ub(a, 0, Cause::Decision).unwrap();
        model.discrete.set_ub(b, 0, Cause::Decision).unwrap();
        assert_eq!(model.boolean_value_of(a), Some(false));
        assert_eq!(model.boolean_value_of(b), Some(false));
        assert!(sat.propagate(&mut model.writer(writer)).is_err());
    }

    #[test]
    fn test_online_clause_insertion() {
        let writer = WriterId::new(1u8);
        let mut model = Model::new();
        let a = model.new_bvar("a");
        let b = model.new_bvar("b");
        let c = model.new_bvar("c");
        let d = model.new_bvar("d");

        let mut sat = SatSolver::new(writer, &mut model);

        let check_values = |model: &Model, values: [Option<bool>; 4]| {
            assert_eq!(model.boolean_value_of(a), values[0], "a");
            assert_eq!(model.boolean_value_of(b), values[1], "b");
            assert_eq!(model.boolean_value_of(c), values[2], "c");
            assert_eq!(model.boolean_value_of(d), values[3], "d");
        };
        check_values(&model, [None, None, None, None]);

        // not(a) and not(b)
        model.discrete.set_ub(a, 0, Cause::Decision).unwrap();
        model.discrete.set_ub(b, 0, Cause::Decision).unwrap();
        check_values(&model, [Some(false), Some(false), None, None]);

        let abcd = vec![a.true_lit(), b.true_lit(), c.true_lit(), d.true_lit()];
        sat.add_clause(&abcd);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [Some(false), Some(false), None, None]);

        let nota_notb = vec![a.false_lit(), b.false_lit()];
        sat.add_clause(&nota_notb);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [Some(false), Some(false), None, None]);

        let nota_b = vec![a.false_lit(), b.true_lit()];
        sat.add_clause(&nota_b);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [Some(false), Some(false), None, None]);

        let a_b_notc = vec![a.true_lit(), b.true_lit(), c.false_lit()];
        sat.add_clause(&a_b_notc);
        sat.propagate(&mut model.writer(writer)).unwrap(); // should trigger and in turn trigger the first clause
        check_values(&model, [Some(false), Some(false), Some(false), Some(true)]);

        let violated = vec![a.true_lit(), b.true_lit(), c.true_lit(), d.false_lit()];
        sat.add_clause(&violated);
        assert!(sat.propagate(&mut model.writer(writer)).is_err());
    }

    #[test]
    fn test_int_propagation() {
        let writer = WriterId::new(1u8);
        let mut model = Model::new();
        let a = model.new_ivar(0, 10, "a");
        let b = model.new_ivar(0, 10, "b");
        let c = model.new_ivar(0, 10, "c");
        let d = model.new_ivar(0, 10, "d");

        let check_values = |model: &Model, values: [(IntCst, IntCst); 4]| {
            assert_eq!(model.domain_of(a), values[0]);
            assert_eq!(model.domain_of(b), values[1]);
            assert_eq!(model.domain_of(c), values[2]);
            assert_eq!(model.domain_of(d), values[3]);
        };
        check_values(&model, [(0, 10), (0, 10), (0, 10), (0, 10)]);

        let mut sat = SatSolver::new(writer, &mut model);
        let clause = vec![ILit::leq(a, 5), ILit::leq(b, 5)];
        sat.add_clause(&clause);
        let clause = vec![ILit::geq(c, 5), ILit::geq(d, 5)];
        sat.add_clause(&clause);

        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [(0, 10), (0, 10), (0, 10), (0, 10)]);

        // lower bound changes

        model.discrete.set_lb(a, 4, Cause::Decision).unwrap();
        check_values(&model, [(4, 10), (0, 10), (0, 10), (0, 10)]);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [(4, 10), (0, 10), (0, 10), (0, 10)]);

        model.discrete.set_lb(a, 5, Cause::Decision).unwrap();
        check_values(&model, [(5, 10), (0, 10), (0, 10), (0, 10)]);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [(5, 10), (0, 10), (0, 10), (0, 10)]);

        // trigger first clause
        model.save_state();
        model.discrete.set_lb(a, 6, Cause::Decision).unwrap();
        check_values(&model, [(6, 10), (0, 10), (0, 10), (0, 10)]);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [(6, 10), (0, 5), (0, 10), (0, 10)]);

        // retrigger first clause with stronger literal
        model.restore_last();
        check_values(&model, [(5, 10), (0, 10), (0, 10), (0, 10)]);
        model.discrete.set_lb(a, 8, Cause::Decision).unwrap();
        check_values(&model, [(8, 10), (0, 10), (0, 10), (0, 10)]);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 10), (0, 10)]);

        // Upper bound changes

        model.discrete.set_ub(c, 6, Cause::Decision).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 6), (0, 10)]);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 6), (0, 10)]);

        model.discrete.set_ub(c, 5, Cause::Decision).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 5), (0, 10)]);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 5), (0, 10)]);

        // should trigger second clause
        model.save_state();
        model.discrete.set_ub(c, 4, Cause::Decision).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 4), (0, 10)]);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 4), (5, 10)]);

        // retrigger second clause with stronger literal
        model.restore_last();
        check_values(&model, [(8, 10), (0, 5), (0, 5), (0, 10)]);
        model.discrete.set_ub(c, 2, Cause::Decision).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 2), (0, 10)]);
        sat.propagate(&mut model.writer(writer)).unwrap();
        check_values(&model, [(8, 10), (0, 5), (0, 2), (5, 10)]);
    }
}
