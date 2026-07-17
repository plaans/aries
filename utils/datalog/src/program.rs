use crate::*;

/// A program is a collection of predicates, facts and rules.
///
/// Running a program with [`Program::run()`] will infer all derivable facts.
#[derive(Default)]
pub struct Program {
    vars: Vec<VarTable>,
    rules: Vec<Box<dyn RuleStep>>,
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

    /// Adds a new rule to the program.
    pub fn add_rule(&mut self, rule: Rule) {
        let steps = rule.decompose(|arity| self.new_predicate(arity));
        for step in steps {
            self.add_rule_step(step);
        }
    }

    fn add_rule_step(&mut self, rule: Box<dyn RuleStep>) {
        self.rules.push(rule);
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
    pub fn run(self) -> Vec<VarTable> {
        while !self.stable() {
            for rule in &self.rules {
                rule.run();
            }

            for var in &self.vars {
                var.process();
            }
        }
        self.vars
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
}
