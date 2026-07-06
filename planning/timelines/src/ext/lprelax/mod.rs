mod encoder;
mod ground;
mod transition;

use std::collections::{HashMap, HashSet};

use aries_env_param::EnvParam;
use aries_solver::core::Var;
use aries_solver::core::views::Term;
use idmap::DirectIdMap;
use itertools::Itertools;

use aries_solver::lang::BoolExpr;
use aries_solver::prelude::{Conjunction, IntCst, Lit};

use aries_lprelax::*;

pub(crate) use encoder::LpRelaxSchedEncoder;
use transition::{Transition, TransitionId};

use crate::ext::Source;
use crate::ext::ground::{SourceGrounding, SourceGroundingFlatId};
use crate::ext::lprelax::ground::{TransitionGrounding, TransitionGroundingFlatId};
use crate::{IntTerm, TaskId};

pub static ARIES_LPRELAX_USE: EnvParam<bool> = EnvParam::new("ARIES_LPRELAX_USE", "false");
static ARIES_LPRELAX_GROUNDER: EnvParam<String> = EnvParam::new("ARIES_LPRELAX_GROUNDER", "simple");

#[derive(Debug)]
pub(crate) struct LpRelaxEncodingConstraint;

impl<'a> BoolExpr<LpRelaxSchedEncoder<'a>> for LpRelaxEncodingConstraint {
    fn enforce_if(&self, l: Lit, ctx: &mut LpRelaxSchedEncoder) {
        assert!(
            l.tautological(),
            "The LP relaxation constraints are defined in the global scope."
        );
        // TODO: assert this being the last constraint encoded ?
        // (equivalently, all other constraints already having been encoded ?)

        assert!(ctx.lprelax.is_none());
        ctx.lprelax = Some(LpRelax::default());

        let mut enc = LpRelaxEncoding::default();
        enc.encode(ctx);

        // enc.print_encoding(ctx);
        enc.print_stats();

        assert!(ctx.lprelax.is_some());
    }

    fn conj_scope(&self, _ctx: &LpRelaxSchedEncoder) -> Conjunction {
        Conjunction::tautology()
    }
}

#[derive(Debug, Default)]
struct LpRelaxEncodingStats {
    relations_time: std::time::Duration,
    build_lp_time: std::time::Duration,
    total_time: std::time::Duration,
}

#[derive(Debug, Default)]
struct LpRelaxEncoding {
    cols: HashMap<ColTag, LpCol>,
    rows: Vec<RowExpr>,

    stats: LpRelaxEncodingStats,
}

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
enum ColTag {
    PresenceSource(Source),
    PresenceSourceGround(Source, SourceGroundingFlatId),
    PresenceTransition(TransitionId),
    PresenceTransitionGround(TransitionId, TransitionGroundingFlatId),
    Support(TransitionId, TransitionId),
    SupportGround(
        TransitionId,
        TransitionId,
        TransitionGroundingFlatId,
        TransitionGroundingFlatId,
    ),
    TermGround(IntTerm, IntCst),
}
#[derive(Debug)]
enum RowExpr {
    Eq(Vec<ColTag>, Vec<ColTag>),
    Leq(Vec<ColTag>, Vec<ColTag>),
    Geq(Vec<ColTag>, Vec<ColTag>),
    Leq1(Vec<ColTag>),
}

impl LpRelaxEncoding {
    fn encode(&mut self, ctx: &mut LpRelaxSchedEncoder) {
        let t0 = std::time::Instant::now();

        let relations = LpRelaxEncodingRelations::from(ctx);
        let (col_tags, row_exprs) = relations.build_col_tags_and_row_exprs(ctx);

        self.stats.relations_time = t0.elapsed();
        let t1 = std::time::Instant::now();

        self._build_lp(col_tags, row_exprs, ctx);
        self._bind_lp_to_main_model(&relations, ctx);

        self.stats.build_lp_time = t1.elapsed();

        self.stats.total_time = t0.elapsed();
    }

    fn _build_lp(&mut self, col_tags: HashSet<ColTag>, row_exprs: Vec<RowExpr>, ctx: &mut LpRelaxSchedEncoder) {
        // Add all columns to the LP problem.

        self.cols = ctx
            .lprelax
            .as_mut()
            .unwrap()
            .add_columns(&vec![(Some(0.), Some(1.)); col_tags.len()])
            .into_iter()
            .zip(col_tags)
            .map(|(col, col_tag)| (col_tag, col))
            .collect();
        self.rows = row_exprs;

        // Add all rows to the LP problem.

        let (mut rows_coefs, mut lbs_ubs) = (vec![], vec![]);
        for row_expr in &self.rows {
            let (row_coefs, lb, ub) = match row_expr {
                RowExpr::Eq(lhs, rhs) => (
                    lhs.iter()
                        .map(|col_tag| (*self.cols.get(col_tag).unwrap(), 1.))
                        .chain(rhs.iter().map(|col_tag| (*self.cols.get(col_tag).unwrap(), -1.)))
                        .collect_vec(),
                    Some(0.),
                    Some(0.),
                ),
                RowExpr::Geq(lhs, rhs) => (
                    lhs.iter()
                        .map(|col_tag| (*self.cols.get(col_tag).unwrap(), 1.))
                        .chain(rhs.iter().map(|col_tag| (*self.cols.get(col_tag).unwrap(), -1.)))
                        .collect_vec(),
                    Some(0.),
                    None,
                ),
                RowExpr::Leq(lhs, rhs) => (
                    lhs.iter()
                        .map(|col_tag| (*self.cols.get(col_tag).unwrap(), 1.))
                        .chain(rhs.iter().map(|col_tag| (*self.cols.get(col_tag).unwrap(), -1.)))
                        .collect_vec(),
                    None,
                    Some(0.),
                ),
                RowExpr::Leq1(lhs) => (
                    lhs.iter()
                        .map(|col_tag| (*self.cols.get(col_tag).unwrap(), 1.))
                        .collect_vec(),
                    None,
                    Some(1.),
                ),
            };
            debug_assert!(
                row_coefs.iter().duplicates_by(|&(col_tag, _)| col_tag).next().is_none(),
                "{:?} {:?}",
                rows_coefs.len(),
                row_expr
            );

            rows_coefs.push(row_coefs);
            lbs_ubs.push((lb, ub));
        }
        ctx.lprelax.as_mut().unwrap().add_rows(&rows_coefs, &lbs_ubs);
    }

    fn _bind_lp_to_main_model(&mut self, relations: &LpRelaxEncodingRelations, ctx: &mut LpRelaxSchedEncoder) {
        // Instead of doing `let lprelax = ctx.lprelax.as_mut().unwrap()`,
        // temporarily take `lprelax` out of the option in ctx, to allow borrowing ctx as immutable (when using `eval`).
        // At the end of this method, put `lprelax` back into the option (`ctx.lprelax = Some(lprelax)`).
        let mut lprelax = ctx.lprelax.take().unwrap();

        let presence_lits_and_cols = {
            let mut res = HashMap::<Lit, HashSet<usize>>::new();
            for &(transition_id, source, source_active) in &relations.presences_lifted_transitions_and_sources {
                res.entry(source_active)
                    .or_default()
                    .insert(self.cols.get(&ColTag::PresenceSource(source)).unwrap().index());

                res.entry(ctx.get_transition_ref(transition_id).get_prez())
                    .or_default()
                    .insert(
                        self.cols
                            .get(&ColTag::PresenceTransition(transition_id))
                            .unwrap()
                            .index(),
                    );
            }
            res
        };

        // Bind lifted presence columns of the LP with corresponding literals in the main CSP.

        for (p, cols) in presence_lits_and_cols {
            if p.tautological() {
                for &col in &cols {
                    lprelax.change_column(LpCol::from(col), Some(1.), None);
                }
            } else if p.absurd() {
                for &col in &cols {
                    lprelax.change_column(LpCol::from(col), None, Some(0.));
                }
            } else {
                let p = p.variable();
                assert!(p != Var::ZERO);

                for &col in &cols {
                    lprelax.add_col_half_binding_default(LpCol::from(col), p);
                }
                lprelax.add_var_half_binding(
                    p,
                    std::sync::Arc::new(move |lit: Lit| {
                        assert_eq!(lit.variable(), p);
                        cols.iter()
                            .map(|&col| LpLit::from_model_lit(LpCol::from(col), lit))
                            .collect()
                    }),
                );
            }
        }

        // Bind term grounding columns of the LP with corresponding literals in the main CSP.

        for (&term, vs) in &relations.terms_ground {
            if term.is_constant() {
                continue;
            }
            let var = term.variable();
            assert!(var != Var::ZERO);

            let mappings: Vec<(usize, IntCst)> = {
                let mut res = vec![];
                for &v in vs {
                    let col = self.cols.get(&ColTag::TermGround(term, v)).unwrap().index();
                    res.push((col, v));
                }
                res
            };
            lprelax.add_var_half_binding(
                var,
                std::sync::Arc::new(move |lit: Lit| {
                    assert_eq!(lit.variable(), var);
                    mappings
                        .iter()
                        .filter_map(|&(col, val)| {
                            (lit.entails(var.lt(val)) || lit.entails(var.gt(val)))
                                .then_some(LpLit::leq(LpCol::from(col), 0))
                        })
                        .collect()
                }),
            );

            for &v in vs {
                let col = *self.cols.get(&ColTag::TermGround(term, v)).unwrap();

                lprelax.add_col_half_binding(
                    col,
                    std::sync::Arc::new(move |lplit: LpLit| {
                        assert_eq!(lplit.col, col);
                        if lplit.tpe == LpLitType::GEQ && lplit.val == 1 {
                            smallvec::smallvec![var.geq(v), var.leq(v)]
                        } else {
                            Default::default()
                        }
                    }),
                );
            }
        }

        // Bind lifted support columns of the LP with corresponding literals in the main CSP.

        for (&(transition1_id, tansition2_id), &s) in &relations.supports_lifted {
            if let Some(s) = s {
                let s = s.variable();
                debug_assert!(s != Var::ZERO);

                let col = *self.cols.get(&ColTag::Support(transition1_id, tansition2_id)).unwrap();

                lprelax.add_var_half_binding_default(s, col);
                lprelax.add_col_half_binding_default(col, s);
            }
        }

        ctx.lprelax = Some(lprelax);
    }

    #[allow(unused)]
    fn print_stats(&self) {
        println!("# LpRelax Encoding");
        println!("## Stats");
        println!("num columns: {:?}", self.cols.len());
        println!("num rows: {:?}", self.rows.len());
        println!("collect_relations time: {:6?}", self.stats.relations_time.as_secs_f64());
        println!(
            "build_cols_and_rows time: {:6?}",
            self.stats.build_lp_time.as_secs_f64()
        );
        println!("total time: {:6?}", self.stats.total_time.as_secs_f64());
    }

    #[allow(unused)]
    fn print_encoding(&self, ctx: &LpRelaxSchedEncoder) {
        println!("# LpRelax Encoding");
        println!("## Transitions");
        for (tr_id, _) in ctx.iter_transitions() {
            println!("{:?} ==== {:?}", tr_id, ctx.get_transition_ref(tr_id));
        }
        println!("## Columns");
        for (col, col_tag) in self.cols.iter().map(|(col_tag, col)| (col, col_tag)).sorted() {
            println!("{col:?} {col_tag:?}");
        }
        println!("## Rows");
        for (i, r) in self.rows.iter().enumerate() {
            println!("Row({i}) {r:?}");
        }
    }
}
#[derive(Debug, Default)]
struct LpRelaxEncodingRelations {
    presences_lifted_transitions_and_sources: Vec<(TransitionId, Source, Lit)>,
    presences_ground_sources_empty: Vec<SourceGroundingFlatId>,
    presences_ground_sources_concrete: DirectIdMap<TaskId, Vec<SourceGroundingFlatId>>,
    presences_ground_transitions: DirectIdMap<TransitionId, Vec<TransitionGroundingFlatId>>,
    presences_ground_transitions_and_sources:
        HashMap<(TransitionId, TransitionGroundingFlatId), Vec<SourceGroundingFlatId>>,

    sources_concrete_groundings_complete: DirectIdMap<TaskId, bool>,

    terms_ground: HashMap<IntTerm, HashSet<IntCst>>,
    terms_ground_sources_empty: HashMap<(IntTerm, IntCst), Vec<SourceGroundingFlatId>>,
    terms_ground_sources_concrete: HashMap<(IntTerm, IntCst), DirectIdMap<TaskId, Vec<SourceGroundingFlatId>>>,
    terms_ground_transitions: HashMap<(IntTerm, IntCst), DirectIdMap<TransitionId, Vec<TransitionGroundingFlatId>>>,

    supports_lifted: HashMap<(TransitionId, TransitionId), Option<Lit>>,
    supports_lifted_inflow: DirectIdMap<TransitionId, Vec<TransitionId>>,
    supports_lifted_outflow_pure: DirectIdMap<TransitionId, Vec<TransitionId>>,
    supports_ground: HashMap<(TransitionId, TransitionId), Vec<(TransitionGroundingFlatId, TransitionGroundingFlatId)>>,
    supports_ground_inflow:
        HashMap<(TransitionId, TransitionGroundingFlatId), Vec<(TransitionId, TransitionGroundingFlatId)>>,
    supports_ground_outflow_pure:
        HashMap<(TransitionId, TransitionGroundingFlatId), Vec<(TransitionId, TransitionGroundingFlatId)>>,
}

impl LpRelaxEncodingRelations {
    fn from(ctx: &LpRelaxSchedEncoder) -> Self {
        let mut res = LpRelaxEncodingRelations::default();

        println!("Started action grounder");
        let sources_groundings = match ARIES_LPRELAX_GROUNDER.get_ref().as_str() {
            "simple" => ctx.run_new_simple_datalog_grounder(),
            "brutal" => ctx.run_new_brutal_grounder(),
            name => panic!("unknown sources grounder '{name:?}'"),
        };
        println!("Ended action grounder");
        for (src, grds) in &sources_groundings {
            println!("|- {src:?}: {}", grds.len());
            println!("{:?}", ctx.get_source(src));
            // for grd in grds {
            //     println!("|    {grd:?}");
            // }
        }

        // Collect lifted presences of transitions and sources,
        // as well as the relations between groundings of transitions, sources, and terms appearing in them.
        for source in ctx.iter_sources() {
            let source_active = ctx.get_source(&source).map(|task| task.presence).unwrap_or(Lit::TRUE);

            for (transition_id, _) in ctx.get_transitions_of_source(&source) {
                res._add_lifted_transition_and_source(transition_id, source, source_active);
            }

            // TODO: complete / incomplete (partial) groundings ?
            res._add_ground_sources(source, sources_groundings.get(&source).unwrap(), true, ctx);
        }
        for (transition_id, _) in ctx.iter_transitions() {
            let transition_groundings = ctx.get_transition_groundings(transition_id);
            res._add_ground_transitions(transition_id, &transition_groundings, ctx);
        }
        // Collect lifted and ground supports between transitions.
        // Note that here (in the LP relaxation), effect transitions are allowed to
        // be supporters of other effect transitions (on the same predicate / state function),
        // which is not the case in our main definition of causal links.
        // In this specific case where the support is between two effects,
        // the "active" literal is None (as this doesn't correspond to a causal link in the main CSP model).
        for ((transition1_id, transition2_id), active) in ctx.iter_supports() {
            debug_assert!(transition1_id != transition2_id);

            res._add_lifted_support(transition1_id, transition2_id, active, ctx);

            let transition1_groundings = ctx.get_transition_groundings(transition1_id);
            let transition2_groundings = ctx.get_transition_groundings(transition2_id);

            for transition1_grounding in &transition1_groundings {
                for transition2_grounding in &transition2_groundings {
                    res._add_ground_support_if_valid(
                        transition1_id,
                        transition1_grounding,
                        transition2_id,
                        transition2_grounding,
                        ctx,
                    );
                }
            }
        }

        res
    }

    fn build_col_tags_and_row_exprs(&self, ctx: &LpRelaxSchedEncoder) -> (HashSet<ColTag>, Vec<RowExpr>) {
        let mut col_tags = HashSet::new();
        let mut row_exprs = vec![];

        // - A transition is active (i.e. present) iff its source is

        for &(transition_id, source, _) in &self.presences_lifted_transitions_and_sources {
            row_exprs.push(RowExpr::Eq(
                vec![ColTag::PresenceTransition(transition_id)],
                vec![ColTag::PresenceSource(source)],
            ));

            col_tags.insert(ColTag::PresenceTransition(transition_id));
            col_tags.insert(ColTag::PresenceSource(source));
        }

        // - A source is active iff one of its groundings is
        //   NOTE: when the source's considered groundings are incomplete, only the "<-" implication can be enforced (Geq instead of Eq)

        for source in ctx.iter_sources() {
            let (source_groundings_ids, complete) = if let Some(task_id) = source {
                let Some(source_groundings_ids) = self.presences_ground_sources_concrete.get(task_id) else {
                    continue;
                };
                (
                    source_groundings_ids,
                    // &self.presences_ground_sources_concrete[task_id],
                    self.sources_concrete_groundings_complete[task_id],
                )
            } else {
                (&self.presences_ground_sources_empty, true)
            };

            let rhs: Vec<_> = source_groundings_ids
                .iter()
                .map(|&source_grounding_id| ColTag::PresenceSourceGround(source, source_grounding_id))
                .collect();
            if !rhs.is_empty() {
                if complete {
                    row_exprs.push(RowExpr::Eq(vec![ColTag::PresenceSource(source)], rhs));
                } else {
                    row_exprs.push(RowExpr::Geq(vec![ColTag::PresenceSource(source)], rhs));
                }
            }

            for &source_grounding_id in source_groundings_ids {
                col_tags.insert(ColTag::PresenceSourceGround(source, source_grounding_id));
            }
        }

        // - A transition is active iff one of its groundings is
        // - A ground transition is active iff one of its source's compatible groundings is
        //   NOTE: when the source's considered groundings are incomplete, only the "<-" implication can be enforced (Geq instead of Eq)

        for (transition_id, _) in ctx.iter_transitions() {
            let Some(transition_groundings_ids) = self.presences_ground_transitions.get(transition_id) else {
                continue;
            };

            let rhs: Vec<_> = transition_groundings_ids
                .iter()
                .map(|&transition_grounding_id| {
                    ColTag::PresenceTransitionGround(transition_id, transition_grounding_id)
                })
                .collect();
            if !rhs.is_empty() {
                row_exprs.push(RowExpr::Eq(vec![ColTag::PresenceTransition(transition_id)], rhs));
            }

            let source = ctx.get_transition_ref(transition_id).get_source();
            let complete = if let Some(task_id) = source {
                self.sources_concrete_groundings_complete[task_id]
            } else {
                true
            };
            for &transition_grounding_id in transition_groundings_ids {
                let compatible_source_groundings_ids = self
                    .presences_ground_transitions_and_sources
                    .get(&(transition_id, transition_grounding_id))
                    .map(|v| v.iter())
                    .unwrap_or_default();

                let rhs: Vec<_> = compatible_source_groundings_ids
                    .map(|&source_grounding_id| ColTag::PresenceSourceGround(source, source_grounding_id))
                    .collect();
                if !rhs.is_empty() {
                    if complete {
                        row_exprs.push(RowExpr::Eq(
                            vec![ColTag::PresenceTransitionGround(transition_id, transition_grounding_id)],
                            rhs,
                        ));
                    } else {
                        row_exprs.push(RowExpr::Geq(
                            vec![ColTag::PresenceTransitionGround(transition_id, transition_grounding_id)],
                            rhs,
                        ));
                    }
                }

                col_tags.insert(ColTag::PresenceTransitionGround(transition_id, transition_grounding_id));
            }
        }

        // - At most one grounding of a term can be active

        for (&term, vs) in &self.terms_ground {
            if !vs.is_empty() {
                row_exprs.push(RowExpr::Leq1(vs.iter().map(|&v| ColTag::TermGround(term, v)).collect()));
            }

            for &v in vs {
                col_tags.insert(ColTag::TermGround(term, v));
            }
        }

        // - A grounding of a term is active iff a transition grounding using it is active
        for (&(term, v), transitions_groundings) in &self.terms_ground_transitions {
            for (transition_id, transition_groundings_ids) in transitions_groundings {
                let rhs: Vec<_> = transition_groundings_ids
                    .iter()
                    .map(|&transition_grounding_id| {
                        ColTag::PresenceTransitionGround(transition_id, transition_grounding_id)
                    })
                    .collect();
                if !rhs.is_empty() {
                    row_exprs.push(RowExpr::Eq(vec![ColTag::TermGround(term, v)], rhs));
                }
            }
        }

        // - A grounding of a term is active iff a source grounding using it is active
        //   NOTE: when the source's considered groundings are incomplete, only the "<-" implication can be enforced (Geq instead of Eq)

        for (&(term, v), source_groundings_ids) in &self.terms_ground_sources_empty {
            let rhs: Vec<_> = source_groundings_ids
                .iter()
                .map(|&source_grounding_id| ColTag::PresenceSourceGround(None, source_grounding_id))
                .collect();
            if !rhs.is_empty() {
                row_exprs.push(RowExpr::Eq(vec![ColTag::TermGround(term, v)], rhs));
            }
        }
        for (&(term, v), map) in &self.terms_ground_sources_concrete {
            for (task_id, source_groundings_ids) in map {
                let source = Some(task_id);
                let complete = if let Some(task_id) = source {
                    self.sources_concrete_groundings_complete[task_id]
                } else {
                    true
                };

                let rhs: Vec<_> = source_groundings_ids
                    .iter()
                    .map(|&source_grounding_id| ColTag::PresenceSourceGround(source, source_grounding_id))
                    .collect();
                if !rhs.is_empty() {
                    if complete {
                        row_exprs.push(RowExpr::Eq(vec![ColTag::TermGround(term, v)], rhs));
                    } else {
                        row_exprs.push(RowExpr::Geq(vec![ColTag::TermGround(term, v)], rhs));
                    }
                }
            }
        }

        // - If a support is active, then both its transitions must be active

        for &(transition1_id, transition2_id) in self.supports_lifted.keys() {
            row_exprs.push(RowExpr::Leq(
                vec![ColTag::Support(transition1_id, transition2_id)],
                vec![ColTag::PresenceTransition(transition1_id)],
            ));
            row_exprs.push(RowExpr::Leq(
                vec![ColTag::Support(transition1_id, transition2_id)],
                vec![ColTag::PresenceTransition(transition2_id)],
            ));

            col_tags.insert(ColTag::Support(transition1_id, transition2_id));
        }

        // - A support is active iff one of its groundings is
        // - If a ground support is active, then its transitions' corresponding groundings must be active

        for (&(transition1_id, transition2_id), groundings) in &self.supports_ground {
            let rhs: Vec<_> = groundings
                .iter()
                .map(|&(transition1_grounding_id, transition2_grounding_id)| {
                    ColTag::SupportGround(
                        transition1_id,
                        transition2_id,
                        transition1_grounding_id,
                        transition2_grounding_id,
                    )
                })
                .collect();
            if !rhs.is_empty() {
                row_exprs.push(RowExpr::Eq(vec![ColTag::Support(transition1_id, transition2_id)], rhs));
            }

            for &(transition1_grounding_id, transition2_grounding_id) in groundings {
                row_exprs.push(RowExpr::Leq(
                    vec![ColTag::SupportGround(
                        transition1_id,
                        transition2_id,
                        transition1_grounding_id,
                        transition2_grounding_id,
                    )],
                    vec![ColTag::PresenceTransitionGround(
                        transition1_id,
                        transition1_grounding_id,
                    )],
                ));
                row_exprs.push(RowExpr::Leq(
                    vec![ColTag::SupportGround(
                        transition1_id,
                        transition2_id,
                        transition1_grounding_id,
                        transition2_grounding_id,
                    )],
                    vec![ColTag::PresenceTransitionGround(
                        transition2_id,
                        transition2_grounding_id,
                    )],
                ));
            }

            for &(transition1_grounding_id, transition2_grounding_id) in groundings {
                col_tags.insert(ColTag::SupportGround(
                    transition1_id,
                    transition2_id,
                    transition1_grounding_id,
                    transition2_grounding_id,
                ));
            }
        }

        // - A transition is present iff it is supported by another one
        //   NOTE: recall that effect transitions are allowed to be supported too by transitions with the same state fluent and any value

        for (transition2_id, transition1_ids) in &self.supports_lifted_inflow {
            let rhs: Vec<_> = transition1_ids
                .iter()
                .map(|&transition1_id| ColTag::Support(transition1_id, transition2_id))
                .collect();
            if !rhs.is_empty() {
                row_exprs.push(RowExpr::Eq(vec![ColTag::PresenceTransition(transition2_id)], rhs));
            }
        }

        // - A ground transition is present iff it is support by another (compatible) one.

        for (&(transition2_id, transition2_grounding_id), transition1_ids) in &self.supports_ground_inflow {
            let rhs: Vec<_> = transition1_ids
                .iter()
                .map(|&(transition1_id, transition1_grounding_id)| {
                    ColTag::SupportGround(
                        transition1_id,
                        transition2_id,
                        transition1_grounding_id,
                        transition2_grounding_id,
                    )
                })
                .collect();
            if !rhs.is_empty() {
                row_exprs.push(RowExpr::Eq(
                    vec![ColTag::PresenceTransitionGround(
                        transition2_id,
                        transition2_grounding_id,
                    )],
                    rhs,
                ));
            }
        }

        // - If a (non-condition) transition is present, then it can support at most one Eff or CondEff transition.

        for (transition1_id, transition2_ids) in &self.supports_lifted_outflow_pure {
            let rhs: Vec<_> = transition2_ids
                .iter()
                .map(|&transition2_id| ColTag::Support(transition1_id, transition2_id))
                .collect();
            if !rhs.is_empty() {
                row_exprs.push(RowExpr::Geq(vec![ColTag::PresenceTransition(transition1_id)], rhs));
            }
        }

        // - If a ground (non-condition) transition is present, then it can support at most one (compatible) Eff or CondEff transition.

        for (&(transition1_id, transition1_grounding_id), transition1_ids) in &self.supports_ground_outflow_pure {
            let rhs: Vec<_> = transition1_ids
                .iter()
                .map(|&(transition2_id, transition2_grounding_id)| {
                    ColTag::SupportGround(
                        transition1_id,
                        transition2_id,
                        transition1_grounding_id,
                        transition2_grounding_id,
                    )
                })
                .collect();
            if !rhs.is_empty() {
                row_exprs.push(RowExpr::Geq(
                    vec![ColTag::PresenceTransitionGround(
                        transition1_id,
                        transition1_grounding_id,
                    )],
                    rhs,
                ));
            }
        }

        // - Two transitions cannot mutually support each other (i.e. forbid trivial cycles)
        // NOTE: This wouldn't work (would be *incorrect*) without *all* initial effects
        //       (including those filtered out in the main encoding due to being detected as useless).
        //       Without them, the inflow constraint ("transition present iff support by another one") could be impossible to satisfy.
        //       Recall that we add these initial effects outside the main encoding, when computing transitions. (see `Transitions`)

        let mut seen = HashSet::new();
        for (transition2_id, transition1_ids) in &self.supports_lifted_inflow {
            for &transition1_id in transition1_ids {
                if seen.contains(&(transition2_id, transition1_id)) {
                    continue;
                }
                if col_tags.contains(&ColTag::Support(transition2_id, transition1_id)) {
                    seen.insert((transition2_id, transition1_id));

                    row_exprs.push(RowExpr::Leq1(vec![
                        ColTag::Support(transition1_id, transition2_id),
                        ColTag::Support(transition2_id, transition1_id),
                    ]));
                }
            }
        }

        (col_tags, row_exprs)
    }

    fn _add_lifted_transition_and_source(&mut self, transition_id: TransitionId, source: Source, source_active: Lit) {
        self.presences_lifted_transitions_and_sources
            .push((transition_id, source, source_active));
    }

    fn _add_ground_sources(
        &mut self,
        source: Source,
        source_groundings: &[SourceGrounding],
        complete: bool,
        ctx: &LpRelaxSchedEncoder,
    ) {
        debug_assert!(complete || source.is_some());

        if let Some(task_id) = source {
            self.sources_concrete_groundings_complete.insert(task_id, complete);
        }
        for source_grounding in source_groundings {
            self._add_ground_source(source, source_grounding, ctx);
        }
    }
    fn _add_ground_source(&mut self, source: Source, source_grounding: &SourceGrounding, ctx: &LpRelaxSchedEncoder) {
        let source_grounding_id = ctx.flatten_source_grounding(source, source_grounding);

        if let Some(task_id) = source {
            if !self.presences_ground_sources_concrete.contains_key(task_id) {
                self.presences_ground_sources_concrete.insert(task_id, vec![]);
            }
            self.presences_ground_sources_concrete[task_id].push(source_grounding_id);
        } else {
            self.presences_ground_sources_empty.push(source_grounding_id);
        }

        for (&(term, _), &v) in ctx.get_source_terms(&source).iter().zip(source_grounding.inner()) {
            if term.is_constant() {
                continue;
            }
            self.terms_ground.entry(term).or_default().insert(v);

            if let Some(task_id) = source {
                let sets = self.terms_ground_sources_concrete.entry((term, v)).or_default();
                if !sets.contains_key(task_id) {
                    sets.insert(task_id, vec![]);
                }
                sets[task_id].push(source_grounding_id);
            } else {
                self.terms_ground_sources_empty
                    .entry((term, v))
                    .or_default()
                    .push(source_grounding_id)
            }
        }

        for (transition_id, _) in ctx.get_transitions_of_source(&source) {
            let transition_grounding_id = {
                let transition_grounding =
                    ctx.build_transition_grounding_from_source_grounding(transition_id, source_grounding);
                ctx.flatten_transition_grounding(transition_id, &transition_grounding)
            };
            self.presences_ground_transitions_and_sources
                .entry((transition_id, transition_grounding_id))
                .or_default()
                .push(source_grounding_id);
        }
    }

    /// Checks whether the transition
    /// (i) comes from a source whose considered groundings are complete
    /// and (ii) the transition grounding wasn't found to correspond to any source grounding.
    /// If so, then the transition grounding can be safely ignored from the encoding.
    ///
    /// Does not affect correctness.
    fn _is_transition_grounding_ignorable(
        &self,
        _transition_id: TransitionId,
        _transition_grounding_id: TransitionGroundingFlatId,
        _ctx: &LpRelaxSchedEncoder,
    ) -> bool {
        false // NOTE: the check seems to be quite costly to perform...

        // if let Some(task_id) = _ctx.get_transition_ref(_transition_id).get_source()
        //     && self.sources_concrete_groundings_complete[task_id]
        //     && self
        //         .presences_ground_transitions_and_sources
        //         .get(&(_transition_id, _transition_grounding_id))
        //         .is_some_and(|known_compatible_source_groundings| known_compatible_source_groundings.is_empty())
        // {
        //     true
        // } else {
        //     false
        // }
    }

    /// Due to the "ignorable" check, must be performed after all ground sources are added.
    fn _add_ground_transitions(
        &mut self,
        transition_id: TransitionId,
        transition_groundings: &[TransitionGrounding],
        ctx: &LpRelaxSchedEncoder,
    ) {
        for transition_grounding in transition_groundings {
            self._add_ground_transition(transition_id, transition_grounding, ctx);
        }
    }
    /// Due to the "ignorable" check, must be performed after all ground sources are added.
    fn _add_ground_transition(
        &mut self,
        transition_id: TransitionId,
        transition_grounding: &TransitionGrounding,
        ctx: &LpRelaxSchedEncoder,
    ) {
        let transition_grounding_id = ctx.flatten_transition_grounding(transition_id, transition_grounding);

        if !self.presences_ground_transitions.contains_key(transition_id) {
            self.presences_ground_transitions.insert(transition_id, vec![]);
        }
        if self._is_transition_grounding_ignorable(transition_id, transition_grounding_id, ctx) {
            return;
        }
        self.presences_ground_transitions[transition_id].push(transition_grounding_id);

        for (&term, &v) in ctx
            .iter_transition_terms(transition_id)
            .zip(transition_grounding.inner())
        {
            if term.is_constant() {
                continue;
            }

            self.terms_ground.entry(term).or_default().insert(v);

            let sets = self.terms_ground_transitions.entry((term, v)).or_default();
            if !sets.contains_key(transition_id) {
                sets.insert(transition_id, vec![]);
            }
            if !sets[transition_id].contains(&transition_grounding_id) {
                sets[transition_id].push(transition_grounding_id);
            }
        }
    }

    fn _add_lifted_support(
        &mut self,
        transition1_id: TransitionId,
        transition2_id: TransitionId,
        active: Option<Lit>,
        ctx: &LpRelaxSchedEncoder,
    ) {
        debug_assert!(transition1_id != transition2_id);
        debug_assert!(!self.supports_lifted.contains_key(&(transition1_id, transition2_id)));
        debug_assert!(!matches!(ctx.get_transition(transition1_id), Transition::Cond(_)));

        self.supports_lifted.insert((transition1_id, transition2_id), active);

        if !self.supports_lifted_inflow.contains_key(transition2_id) {
            self.supports_lifted_inflow.insert(transition2_id, vec![]);
        }
        self.supports_lifted_inflow[transition2_id].push(transition1_id);

        if matches!(
            ctx.get_transition(transition2_id),
            Transition::Eff(_) | Transition::CondEff(_, _)
        ) {
            if !self.supports_lifted_outflow_pure.contains_key(transition1_id) {
                self.supports_lifted_outflow_pure.insert(transition1_id, vec![]);
            }
            self.supports_lifted_outflow_pure[transition1_id].push(transition2_id);
        }
    }

    /// Due to the "ignorable" check, must be performed after all ground sources are added.
    fn _add_ground_support_if_valid(
        &mut self,
        transition1_id: TransitionId,
        transition1_grounding: &TransitionGrounding,
        transition2_id: TransitionId,
        transition2_grounding: &TransitionGrounding,
        ctx: &LpRelaxSchedEncoder,
    ) {
        debug_assert!(transition1_id != transition2_id);
        debug_assert!(!matches!(ctx.get_transition(transition1_id), Transition::Cond(_)));

        // let transition1_grounding_id = ctx.flatten_transition_grounding(transition1_id, transition1_grounding);
        // let transition2_grounding_id = ctx.flatten_transition_grounding(transition2_id, transition2_grounding);
        //
        // if self._is_transition_grounding_ignorable(transition1_id, transition1_grounding_id, ctx)
        //     || self._is_transition_grounding_ignorable(transition2_id, transition2_grounding_id, ctx)
        // {
        //     return;
        // }

        let ground_support_valid = {
            let n = ctx.get_transition_ref(transition1_id).get_args().len();
            debug_assert!(n == ctx.get_transition_ref(transition2_id).get_args().len());

            if transition1_grounding.inner()[..n] != transition2_grounding.inner()[..n] {
                // incompatible ground args
                false
            } else {
                // compatible ground values
                match (
                    ctx.get_transition_ref(transition1_id).get_valto_term_idx(),
                    ctx.get_transition_ref(transition2_id).get_valfrom_term_idx(),
                ) {
                    (Some(i), Some(j)) => transition1_grounding.inner()[i] == transition2_grounding.inner()[j],
                    (Some(_), None) => true,
                    (None, _) => unreachable!(),
                }
            }
        };
        if !ground_support_valid {
            return;
        }

        let transition1_grounding_id = ctx.flatten_transition_grounding(transition1_id, transition1_grounding);
        let transition2_grounding_id = ctx.flatten_transition_grounding(transition2_id, transition2_grounding);

        self.supports_ground
            .entry((transition1_id, transition2_id))
            .or_default()
            .push((transition1_grounding_id, transition2_grounding_id));

        self.supports_ground_inflow
            .entry((transition2_id, transition2_grounding_id))
            .or_default()
            .push((transition1_id, transition1_grounding_id));

        if matches!(
            ctx.get_transition(transition2_id),
            Transition::Eff(_) | Transition::CondEff(_, _)
        ) {
            self.supports_ground_outflow_pure
                .entry((transition1_id, transition1_grounding_id))
                .or_default()
                .push((transition2_id, transition2_grounding_id));
        }
    }
}
