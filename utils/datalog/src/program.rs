use std::collections::HashMap;

use crate::*;

/// A program is a collection of predicates, facts and rules.
///
/// Running a program with [`Program::run()`] will infer all derivable facts.
#[derive(Default)]
pub struct Program {
    vars: Vec<VarTable>,
    rules: Vec<Box<dyn RuleStep>>,

    /// For a given symbol (key) stores the index of the interned / cached singleton predicate / table.
    symbol_predicates_cache: HashMap<u32, usize>,
}

impl Program {
    /// Creates a new, empty, program.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a new predicate in th program.
    ///
    /// Fact for this predicate can be added on the returned [`VarTable`].
    pub fn new_predicate(&mut self, arity: usize) -> VarTable {
        let table = VarTable::new(arity);
        self.vars.push(table.clone());
        table
    }

    /// Number of predicates that have been added to the program.
    pub fn num_predicates(&self) -> usize {
        self.vars.len()
    }
    /// Returns a reference to the [`VarTable`] of the i-th predicate added to the program.
    pub fn get_predicate(&self, i: usize) -> Option<&VarTable> {
        self.vars.get(i)
    }
    /// Returns a reference to the [`VarTable`] of the i-th predicate added to the program.
    pub fn get_predicate_mut(&mut self, i: usize) -> Option<&mut VarTable> {
        self.vars.get_mut(i)
    }

    /// Adds a new rule to the program after normalizing it.
    pub fn add_rule(&mut self, rule: Rule) {
        let rule = self.normalize_rule(rule);

        let steps = rule.decompose(|arity| self.new_predicate(arity));
        for step in steps {
            self.add_rule_step(step);
        }
    }

    fn add_rule_step(&mut self, rule: Box<dyn RuleStep>) {
        self.rules.push(rule);
    }

    /// Returns a new rule without symbols (constants) in the head (required by the internal rule representation)
    /// after potentially adding new predicates and facts.
    ///
    /// More precisely, works by taking symbols (constants) in the head and "promoting" them to the body:
    /// each symbol `Arg::Sym(c)` in the head is replaced by a fresh variable `Arg::Var(v)`
    /// and an atom `singleton_c(Arg::Var(v))` is appended to the body.
    ///
    /// `singleton_c` is a fresh arity-1 predicate that is interned (cached)
    /// per distinct `Arg::Sym(c)` and seeded with the single fact `singleton_c(c)`.
    /// This is why this method borrows `self` as mutable.
    ///
    /// If `Arg::Sym(c)` is the `i`-th encountered constant in the head, then `v` is chosen to be
    /// the `i`-th integer in descending order starting from the maximum allowable value for `v`.
    /// (Remember that this value only matters locally to the rule, not globally in the program)
    fn normalize_rule(&mut self, rule: Rule) -> Rule {
        #[cfg(debug_assertions)]
        {
            let max_rulelocal_var = rule
                .head
                .args()
                .chain(rule.body.iter().flat_map(|a| a.args()))
                .filter_map(|a| match a {
                    Arg::Var(v) => Some(v),
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            let num_to_promote = rule.head.args().filter(|a| matches!(a, Arg::Sym(_))).count() as u32;
            assert!(
                max_rulelocal_var + num_to_promote < i32::MAX as u32,
                "there are not enough fresh (local) var ids to promote symbols to ({max_rulelocal_var:?}, {num_to_promote:?})"
            );
        }

        let mut fresh = (i32::MAX - 1) as u32;
        let mut extra_body = Vec::new();
        let rewritten_args: Vec<Arg> = rule
            .head
            .args()
            .map(|arg| match arg {
                Arg::Sym(c) => {
                    let cached_symbol_predicate = if let Some(&i) = self.symbol_predicates_cache.get(&c) {
                        assert!(self.vars[i].arity() == 1, "cached singleton predicate has non-1 arity");
                        self.get_predicate(i).unwrap()
                    } else {
                        let i = self.num_predicates();
                        let res = self.new_predicate(1);
                        self.symbol_predicates_cache.insert(c, i);
                        res.add([c]);
                        self.get_predicate(i).unwrap()
                    };
                    let id = fresh;
                    fresh = fresh.checked_sub(1).expect("fresh-id underflow");
                    extra_body.push(cached_symbol_predicate.apply([Arg::Var(id)]));
                    Arg::Var(id)
                }
                v @ Arg::Var(_) => v,
            })
            .collect();

        let new_head = rule.head.predicate().apply(rewritten_args);
        let new_body = {
            let mut res = rule.body;
            res.append(&mut extra_body);
            res
        };

        Rule::new(new_head, new_body)
    }

    /// Returns true if the program is stable (i.e. inference has reached a fixed-point).
    pub fn stable(&self) -> bool {
        self.vars.iter().all(|var| var.stable())
    }

    /// Runs the program, which will repeatedly trigger all rules until a fixed-point is reached.
    ///
    /// All [`VarTable`] will be updated into a stable form and the resulting facts can be extracted with [`VarTable::extract()`].
    ///
    /// This method consumes the objects as it would be a logic error to modify it again
    /// (e.g. adding new rules and running again would be a no-op because all fact would be stable already).
    pub fn run(self) {
        while !self.stable() {
            for rule in &self.rules {
                rule.run();
            }

            for var in &self.vars {
                var.process();
            }
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{Arg, Rule, program::Program};

    #[test]
    fn test_grounding() {
        let mut prog = Program::new();

        let loc = prog.new_predicate(1);
        loc.add([1]);
        loc.add([2]);
        loc.add([3]);
        loc.add([4]);
        loc.add([6]);
        loc.add([7]);

        let robot = prog.new_predicate(1);
        robot.add([11]);
        robot.add([12]);
        robot.add([13]);
        robot.add([14]);
        robot.add([16]);

        let connected = prog.new_predicate(2);
        connected.add([1, 2]);
        connected.add([2, 3]);
        connected.add([3, 4]);
        connected.add([1, 2]);
        connected.add([2, 1]);
        connected.add([3, 2]);
        connected.add([4, 3]);
        connected.add([2, 1]);
        connected.add([6, 7]);
        connected.add([7, 6]);

        let at = prog.new_predicate(2);
        at.add([11, 2]);
        at.add([12, 4]);
        at.add([16, 7]);

        use Arg::*;

        let move_applicable = prog.new_predicate(3);

        let move_rule = Rule::new(
            move_applicable.apply([Var(0), Var(1), Var(2)]),
            [
                robot.apply([Var(0)]),
                loc.apply([Var(1)]),
                loc.apply([Var(2)]),
                at.apply([Var(0), Var(1)]),
                connected.apply([Var(1), Var(2)]),
            ],
        );
        prog.add_rule(move_rule);

        prog.add_rule(Rule::new(
            at.apply([Var(0), Var(2)]),
            [move_applicable.apply([Var(0), Var(1), Var(2)])],
        ));

        prog.run();

        at.stable.borrow().rows().for_each(|row| println!("at{row:?}"));

        move_applicable
            .stable
            .borrow()
            .rows()
            .for_each(|row| println!("move{row:?}"));
    }

    #[test]
    fn test_normalize_rule() {
        let mut prog = Program::new();

        let p = prog.new_predicate(2);
        let q = prog.new_predicate(2);
        let r = prog.new_predicate(1);

        q.add([1, 10]);
        q.add([2, 20]);
        r.add([10]);

        use Arg::*;

        // p(1, ?x) :- q(?x, ?y), r(?y)
        // Expect: p(1, 1) derived
        prog.add_rule(Rule::new(
            p.apply([Sym(1), Var(0)]),
            [q.apply([Var(0), Var(1)]), r.apply([Var(1)])],
        ));

        prog.run();

        let p_facts: Vec<Vec<_>> = p.stable.borrow().rows().map(|r| r.to_vec()).collect();
        assert_eq!(p_facts, vec![vec![1, 1]]);
    }
}
