use std::fmt::{Debug, Formatter};
use std::sync::Arc;

use itertools::Itertools;

use crate::core::literals::{ConjunctionBuilder, DisjunctionBuilder};
use crate::lang::{ModelView, VarCst};
use crate::prelude::*;

/// A set of tuples, representing the allowed values in a table constraint.
///
/// Each row represent an allowed assignment to the variables.
#[derive(Clone)]
pub struct Table<E> {
    /// Number of elements in the tuple, corresponding to the number of variables in a table constraint.
    num_columns: usize,
    /// Flat representation of a matrix (each line occurs right after the previous one)
    inner: Vec<E>,
}

impl<E> Debug for Table<E> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "table({})", self.num_columns)
    }
}

impl<E: Clone> Table<E> {
    pub fn new(num_columns: usize) -> Table<E> {
        Table {
            num_columns,
            inner: Vec::new(),
        }
    }

    pub fn push_line(&mut self, line: &[E]) {
        assert_eq!(line.len(), self.num_columns);
        self.inner.extend_from_slice(line);
    }

    pub fn lines(&self) -> impl Iterator<Item = &[E]> {
        self.inner.chunks(self.num_columns)
    }

    pub fn columns(&self) -> impl Iterator<Item = Vec<&E>> {
        (0..self.num_columns).map(move |i| self.inner.iter().skip(i).step_by(self.num_columns).collect())
    }
}

/// Constraint that enforces a tuple of variables to take their values in a specified number of allowed assignments.
///
/// ## Optionals
///
/// The constraint is in scope when *all* the variables are.
/// This means that we do *not* ignore absent variables: if a variable is absent, then
/// the entire constraint is undefined.
#[derive(Clone, Debug)]
pub struct InTable {
    variables: Vec<VarCst>,
    value_tuples: Arc<Table<IntCst>>,
}

impl InTable {
    pub fn new(variables: Vec<VarCst>, allowed_assignments: Arc<Table<IntCst>>) -> Self {
        assert_eq!(variables.len(), allowed_assignments.num_columns);
        InTable {
            variables,
            value_tuples: allowed_assignments,
        }
    }
}

impl<Ctx: ModelView> BoolExpr<Ctx> for InTable {
    fn enforce_if(&self, implicant: Lit, ctx: &mut Ctx) {
        let mut supported_by_a_line: Vec<Lit> = Vec::with_capacity(256);

        let vars = &self.variables;
        let table = self.value_tuples.as_ref();
        let model = ctx;

        let mut lines = Vec::new();
        for tuple in table.lines() {
            // for each line i, we create a `sup_i` literal such that, if sup_i
            // is true, the variables takes the value in this line

            // sup_i => vars[k] == tuple_i[k],   for all variable indices k

            assert_eq!(vars.len(), tuple.len());
            let mut supported_by_this_line = ConjunctionBuilder::with_capacity(tuple.len() * 2);
            for (&var, &val) in vars.iter().zip(tuple.iter()) {
                supported_by_this_line.push(model.half_reify(leq(var, val)));
                supported_by_this_line.push(model.half_reify(geq(var, val)));
            }
            let support = model.half_reify(and(supported_by_this_line));
            lines.push((support, tuple));
            supported_by_a_line.push(support);
        }

        // enforce that at least one line matches the variable values
        // implicant => Or { sup_i | i in tuples indices }
        model.enforce_if(implicant, or(supported_by_a_line));

        for (k, var) in vars.iter().copied().enumerate() {
            // for a given variable var = vars[k]

            // all values allowed for var
            let allowed_values = lines.iter().map(|(_, values)| values[k]).collect_vec();
            model.enforce_if(implicant, HasValueIn::new(var, allowed_values));

            // zipped supports and values for vars[k] : [ (sup_i, tuples_i[k]) ]
            let val_supports = lines
                .iter()
                .map(|(support, values)| (*support, values[k]))
                .collect_vec();

            let values = val_supports
                .iter()
                .map(|(_, val)| *val)
                .sorted_unstable()
                .dedup()
                .collect_vec();
            for &n in &values {
                // var > n  =>  or_i { sup_i | tuple_i[k] > n }
                let mut ge_clause = DisjunctionBuilder::new();
                ge_clause.push(!var.gt_lit(n));
                // var < n  =>  or { sup_i | tuple_i[k] < n }
                let mut le_clause = DisjunctionBuilder::new();
                le_clause.push(!var.lt_lit(n));

                for &(support, val) in &val_supports {
                    if val > n {
                        ge_clause.push(support);
                    }
                    if val < n {
                        le_clause.push(support);
                    }
                }
                model.enforce_if(implicant, or(ge_clause));
                model.enforce_if(implicant, or(le_clause));
            }
        }
    }

    fn conj_scope(&self, ctx: &Ctx) -> Conjunction {
        Conjunction::from_iter(self.variables.iter().map(|var| ctx.presence(*var)))
    }
}

/// Constraint that explicitly defines the allowed values for a variable.
/// This is primarily useful when the domain of a variable has holes in it.
#[derive(Debug, Clone)]
pub struct HasValueIn {
    /// Variable on which the constraint is placed
    variable: VarCst,
    /// Values that are allowed for this variable.
    /// The vector is expected to be sorted (and ideally deduplicated)
    allowed_values: Vec<IntCst>,
}

impl HasValueIn {
    pub fn new(variable: VarCst, mut allowed_values: Vec<IntCst>) -> Self {
        allowed_values.sort();
        allowed_values.dedup();
        Self {
            variable,
            allowed_values,
        }
    }
}

impl<Ctx: ModelView> BoolExpr<Ctx> for HasValueIn {
    fn enforce_if(&self, implicant: Lit, ctx: &mut Ctx) {
        debug_assert!(self.allowed_values.is_sorted());

        if self.allowed_values.is_empty() {
            ctx.enforce_if(implicant, Lit::FALSE);
            return;
        }

        let min = *self.allowed_values.first().unwrap();
        let max = *self.allowed_values.last().unwrap();
        ctx.enforce_if(implicant, self.variable.ge_lit(min));
        ctx.enforce_if(implicant, self.variable.le_lit(max));

        let mut prev = min;
        for &val in self.allowed_values.iter().skip(1) {
            if prev != val - 1 {
                // there is a hole in the domain
                // [..., prev] U [val, ..]
                ctx.enforce_if(implicant, or([self.variable.le_lit(prev), self.variable.ge_lit(val)]));
            }
            prev = val
        }
    }

    fn conj_scope(&self, ctx: &Ctx) -> Conjunction {
        ctx.presence(self.variable).into()
    }
}

#[cfg(test)]
mod test {

    use rand::{Rng, SeedableRng, rngs::SmallRng};

    use super::*;

    #[test]
    fn simple_test() {
        let mut table = Table::new(3);
        table.push_line(&[2, 3, 6]);
        table.push_line(&[4, 3, 6]);
        table.push_line(&[2, 5, 6]);
        test_table(table);
    }

    #[test]
    fn test_random_tables() {
        for i in 0..10 {
            test_random_table(i);
        }
    }

    fn test_random_table(seed: u64) {
        let mut rng = SmallRng::seed_from_u64(seed);
        let num_vars = rng.random_range(1..10);
        let num_lines = rng.random_range(0..50);

        println!("\n== Table test ({num_vars} x {num_lines}) ==\n");

        let mut table = Table::new(num_vars);
        for _ in 0..num_lines {
            let line: Vec<IntCst> = (0..num_vars).map(|_| rng.random_range(-10..10)).collect_vec();
            table.push_line(&line);
        }

        test_table(table);
    }

    fn test_table(table: Table<IntCst>) {
        let table = Arc::new(table);
        let mut model = Model::new();
        let vars = (0..table.num_columns)
            .map(|_| model.new_variable(INT_CST_MIN, INT_CST_MAX))
            .collect_vec();

        model.enforce(in_table(vars.clone(), table.clone()));

        let mut solver = Solver::new(model);
        solver.set_brancher(crate::solver::search::random::RandomChoice::new(1));

        let Ok(mut assignments) = solver.enumerate(&vars, SearchLimit::None) else {
            assert_eq!(table.lines().count(), 0);
            return;
        };
        solver.print_stats();

        assignments.sort();
        assignments.dedup();
        for ass in &assignments {
            println!("{ass:?}");
        }

        let expected_assignments = table.lines().sorted().dedup().map(Vec::from).collect_vec();

        assert_eq!(assignments, expected_assignments);
    }
}
