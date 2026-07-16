use std::{
    cmp::Reverse,
    collections::{HashMap, HashSet},
};

use itertools::Itertools;

use crate::*;

pub(crate) trait RuleStep {
    fn run(&self);
}

/// An element of a rules of the form, e.g., `parent(?x, a)`.
#[derive(Clone)]
pub struct RuleAtom {
    predicate: VarTable,
    args: Pattern,
}

impl RuleAtom {
    /// Creates a new [`RuleAtom`] for a given predicate.
    ///
    /// One should consider using the more convenient [`VarTable::apply`] method for this purpose.
    pub fn new(predicate: VarTable, args: impl Into<Pattern>) -> Self {
        let args = args.into();
        assert_eq!(predicate.arity(), args.arity());
        Self { predicate, args }
    }
}

/// A rule in a datalog program, e.g. `ancestor(?x, ?z) :- parent(?x, ?y), ancestor(?y, ?z)`.
#[derive(Clone)]
pub struct Rule {
    head: RuleAtom,
    body: Vec<RuleAtom>,
}

impl Rule {
    /// Creates an new rule from an head (single atom left of `:-`) and a body (conjunction of atoms on the right-side)
    pub fn new(head: RuleAtom, body: impl AsRef<[RuleAtom]>) -> Self {
        Self {
            head,
            body: body.as_ref().iter().cloned().collect_vec(),
        }
    }
}

impl Rule {
    /// Optimizes a rule by reordering the atoms of the body for more efficient evaluation
    fn optimize(&mut self) {
        // extract body and replace with empty vec
        // we will move back all its elements in an order that is better suited for evaluation
        let mut body = std::mem::take(&mut self.body);

        while !body.is_empty() {
            // vars that are already bound by patterns in the current body (self.body)
            // when evaluating any of the remaining atoms in `body`, those vars will be already bound to a constant
            let bound_vars: HashSet<_> = self.body.iter().flat_map(|p| p.args.vars()).collect();

            // priority of an atom, determined by:
            //  - minimal number of free variables (highest priority)
            //  - maximal number of bound variables (tie breaking)
            let priority = |atom: &&RuleAtom| {
                let num_free_vars = atom.args.vars().unique().filter(|v| !bound_vars.contains(v)).count();
                let num_vars = atom.args.vars().unique().count();
                (num_free_vars, Reverse(num_vars))
            };

            // select the atom with highest priority and move it to the body
            let min_index = body.iter().position_min_by_key(priority).unwrap();
            let min = body.remove(min_index);
            self.body.push(min);
        }
    }

    /// Decomposes a rule into simpler inference steps.
    pub(crate) fn decompose(&self, mut new_var: impl FnMut(usize) -> VarTable) -> Vec<Box<dyn RuleStep>> {
        let mut out: Vec<Box<dyn RuleStep>> = Vec::new();

        let mut rule = self.clone();
        rule.optimize();
        let Rule { head, mut body } = rule;

        while body.len() > 2 {
            // combine the first two atoms into a single one, on a synthetic predicate
            let pat1 = body.remove(0);
            let pat2 = body.remove(0);
            let vars = pat1.args.vars().chain(pat2.args.vars()).unique().collect_vec();
            // TODO: retain only those appearing in head or the rest of the body
            let temp_var = new_var(vars.len());
            let temp_atom = RuleAtom {
                predicate: temp_var,
                args: Pattern::new(vars.iter().map(|var| Arg::Var(*var)).collect_vec()),
            };
            out.push(Box::new(JoinRule::new(temp_atom.clone(), [pat1, pat2])));
            // put the new atom at the beginning of the body
            body.insert(0, temp_atom);
        }

        match &body[..] {
            [] => panic!(
                "Rule with empty body. No variables are allowed and it thus should be a fact, added directly to the VarTable."
            ),
            [pat] => {
                // rule with a single atom in the body, which mostly selects/reorder a subset of the parameters
                // Since we can only manage joins, we join with a dummy atom that will always match exactly once
                let tautological_table = new_var(1);
                tautological_table.add([0]);
                out.push(Box::new(JoinRule::new(
                    head,
                    [pat.clone(), tautological_table.apply([Arg::Sym(0)])],
                )));
            }
            [pat1, pat2] => {
                out.push(Box::new(JoinRule::new(head, [pat1.clone(), pat2.clone()])));
            }
            _ => unreachable!(),
        }

        out
    }
}

pub(crate) struct JoinRule {
    join: Join,
    table1: VarTable,
    table2: VarTable,
    out_table: VarTable,
}

impl RuleStep for JoinRule {
    fn run(&self) {
        self.join.run(
            &self.table1.recent.borrow(),
            &self.table2.recent.borrow(),
            &mut self.out_table.to_add.borrow_mut(),
        );
        self.join.run(
            &self.table1.stable.borrow(),
            &self.table2.recent.borrow(),
            &mut self.out_table.to_add.borrow_mut(),
        );
        self.join.run(
            &self.table1.recent.borrow(),
            &self.table2.stable.borrow(),
            &mut self.out_table.to_add.borrow_mut(),
        );
    }
}

impl JoinRule {
    pub fn new(mut head: RuleAtom, body: [RuleAtom; 2]) -> Self {
        let mut vars = HashMap::new();
        let mut next_var_id: u32 = 0;

        let [mut body1, mut body2] = body;

        for pat in [&mut head.args, &mut body1.args, &mut body2.args] {
            pat.map_vars(|var| {
                *vars.entry(var).or_insert_with(|| {
                    next_var_id += 1;
                    next_var_id - 1
                })
            });
        }
        assert_eq!(
            head.args.pattern,
            (0..(head.args.pattern.len()))
                .map(|i| Pattern::from_var(i as u32))
                .collect_vec()
        );

        Self {
            join: Join::new(head.args.pattern.len(), body1.args, body2.args),
            table1: body1.predicate,
            table2: body2.predicate,
            out_table: head.predicate,
        }
    }
}

pub(crate) struct Join {
    output_arity: usize,
    num_vars: usize,
    pattern1: Pattern,
    pattern2: Pattern,
}

impl Join {
    pub(crate) fn new(output_arity: usize, pattern1: Pattern, pattern2: Pattern) -> Self {
        let vars = pattern1
            .pattern
            .iter()
            .chain(pattern2.pattern.iter())
            .filter_map(|&i| if i < 0 { Some((-i - 1) as usize) } else { None })
            .sorted()
            .dedup()
            .collect_vec();
        let num_vars = vars.len();
        assert_eq!(vars, (0..num_vars).collect_vec());
        assert!(output_arity <= num_vars);
        Self {
            output_arity,
            num_vars,
            pattern1,
            pattern2,
        }
    }

    pub fn run(&self, table1: &Table, table2: &Table, out: &mut TableBuff<Sym>) {
        if table1.is_empty() || table2.is_empty() {
            return;
        }
        let empty_bindings = (0..self.num_vars).map(|_| Sym::MAX).collect_vec();
        let binding = self.pattern1.all_bindings(table1, &empty_bindings);
        for partial_binding in binding.rows() {
            let pattern2 = self.pattern2.specialized(partial_binding);

            let full_bindings = pattern2.all_bindings(table2, partial_binding);
            for row in full_bindings.rows() {
                assert!(row.iter().all(|i| *i != Sym::MAX), "some unbound variables");
                let output_variables = &row[..self.output_arity];
                out.push(output_variables);
            }
        }
    }
}

/// An argument in a rule
#[derive(Clone, Copy, Debug)]
pub enum Arg {
    /// A variable with a given ID
    Var(u32),
    /// A symbol (with its numeric representation)
    Sym(Sym),
}

/// A pattern to match on a table, composed of constants and variables.
///
/// For instance, a pattern `[?x, c, ?y]` can be matched on a table
///
/// - `[a, b, c]`
/// - `[a, c, b]`
/// - `[b, a, c]`
///
/// in which it would only match the second row `[a, c, b]` (which `?x=a` and `?y=b`).
///
#[derive(Clone)]
pub struct Pattern {
    /// Encoding of the pattern, where constant are represented with positive values and variables with negative values.
    ///
    /// For conversions from/into this representation, see: [Self::as_var]  [Self::from_var]  [Self::as_cst] [Self::from_cst]
    pattern: Vec<i32>,
    /// If a variable appears more than once in the pattern, then, this will contain all pairs of indices in the pattern that must be checked for equality.
    ///
    /// For instance, a pattern, `[a, ?x, ?y ?x, ?x, ?y]` will have equalities: `[1, 3]`, `[3, 4]` and `[2, 5]`
    /// Note that the pair `[1, 4]` is not present checking would be redundant.
    ///
    /// This information is redundant with the pattern itself but allows for more efficient matches as the
    /// elements which must be equal are precomputed.
    equalities: Vec<[usize; 2]>,
}
impl Pattern {
    /// Creates a new pattern.
    ///
    /// For instance the following would create a pattern of the form `[?x, c, ?y]` where `c` is a symbol
    /// encoded with the value 3.
    /// ```
    /// use aries_datalog::*;
    /// Pattern::new([Arg::Var(0), Arg::Sym(3), Arg::Var(1)]);
    /// ```
    pub fn new(pattern: impl AsRef<[Arg]>) -> Self {
        // first, detect all duplicated variables to fill the equality table
        let indices_of_vars = pattern
            .as_ref()
            .iter()
            .enumerate()
            .filter_map(|(i, q)| match q {
                Arg::Var(x) => Some((*x, i)),
                Arg::Sym(_) => None,
            })
            .into_group_map();
        let mut equalities = Vec::new();
        for (_var, indices) in indices_of_vars {
            for (idx1, idx2) in indices.iter().tuple_windows() {
                equalities.push([*idx1, *idx2]);
            }
        }
        // then simply encode the pattern
        Self {
            pattern: pattern
                .as_ref()
                .iter()
                .map(|qa| match qa {
                    Arg::Var(i) => Self::from_var(*i),
                    Arg::Sym(s) => Self::from_cst(*s),
                })
                .collect_vec(),
            equalities,
        }
    }

    /// Number of elements in the pattern.
    pub fn arity(&self) -> usize {
        self.pattern.len()
    }

    fn as_var(term: i32) -> Option<u32> {
        (term < 0).then(|| (-term - 1) as u32)
    }
    fn from_var(var: u32) -> i32 {
        -(var as i32) - 1
    }
    fn from_cst(cst: u32) -> i32 {
        cst as i32
    }

    pub(crate) fn vars(&self) -> impl Iterator<Item = u32> + '_ {
        self.pattern.iter().copied().filter_map(Self::as_var)
    }
    pub(crate) fn map_vars(&mut self, mut f: impl FnMut(u32) -> u32) {
        for i in &mut self.pattern {
            if let Some(var) = Self::as_var(*i) {
                *i = Self::from_var(f(var))
            }
        }
    }

    /// Creates a new pattern where some variables are bound to a constant value.
    ///
    /// The partial binding `[4, Sym::MAX, 7]` would replace `Var(0)` with 4 and `Var(2)` with 7.
    /// The `Sym::MAX` indicates an empty cell, meaning `Var(1)` would be kept.
    pub(crate) fn specialized(&self, partial_var_bindings: &[Sym]) -> Pattern {
        Pattern {
            pattern: self
                .pattern
                .iter()
                .map(|&x| {
                    if let Some(var) = Self::as_var(x) {
                        let var = var as usize;
                        let value = partial_var_bindings[var];
                        if value != Sym::MAX {
                            Self::from_cst(value) // we have a value for this variable
                        } else {
                            x // no value for this variable, keep the variable
                        }
                    } else {
                        x // a constant, retain
                    }
                })
                .collect_vec(), // TODO: potentially called in semi-hot loops
            equalities: self.equalities.clone(),
        }
    }

    /// Gets an arity-specialized version.
    fn compiled<'me, const N: usize>(&'me self) -> PatternN<'me, N> {
        assert_eq!(self.pattern.len(), N);
        let pattern = self.pattern.first_chunk().unwrap();
        // smallest/biggest matches, by replacing variables by the smallest/biggest symbols
        let min = pattern.map(|p| {
            if let Some(_var) = Self::as_var(p) {
                Sym::MIN
            } else {
                p as Sym
            }
        });
        let max = pattern.map(|p| {
            if let Some(_var) = Self::as_var(p) {
                Sym::MAX
            } else {
                p as Sym
            }
        });
        PatternN {
            pattern,
            equalities: self.equalities.as_slice(),
            min,
            max,
        }
    }

    /// Gathers all bindings of this pattern of the given table.
    /// THe `out` slice, will be used as a base and partially overwritten by variables.
    ///
    /// - pattern: `[Var(0), Var(1), Sym(3)]`
    /// - table:
    ///   - `[1, 2, 3]`
    ///   - `[1, 2, 2]`
    ///   - `[1, 3, 3]`
    /// - out: `[0, 0, 9]`
    ///
    /// will produce the outputs:
    ///
    ///  - `[1, 2, 9]`
    ///  - `[1, 3, 9]`
    ///
    fn all_bindings<'a>(&'a self, table: &'a Table, out: &'a [u32]) -> TableBuff<u32> {
        match self.pattern.len() {
            0 => {
                debug_assert_eq!(table.num_columns(), 0);
                let mut buff = TableBuff::new(out.len());
                if !table.is_empty() {
                    // table contains a single element (the unit row, with no columns)
                    // since the row is unit, it
                    // 1) necessarily matches the pattern (no data to filter anything)
                    // 2) will not bind any variable (no data to bind anything)
                    buff.push(out);
                }
                buff
            }
            1 => self.compiled::<1>().all_bindings(table.rows_sized(), out),
            2 => self.compiled::<2>().all_bindings(table.rows_sized(), out),
            3 => self.compiled::<3>().all_bindings(table.rows_sized(), out),
            4 => self.compiled::<4>().all_bindings(table.rows_sized(), out),
            5 => self.compiled::<5>().all_bindings(table.rows_sized(), out),
            _ => {
                let mut bindings = TableBuff::new(out.len());
                let mut out_row = Vec::from(out);
                for row in table.rows() {
                    if self.matches(row) {
                        self.bind(row, &mut out_row);
                        bindings.push(&out_row);
                    }
                }
                bindings
            }
        }
    }

    fn matches(&self, data: &[Sym]) -> bool {
        debug_assert_eq!(self.pattern.len(), data.len());
        for &[id1, id2] in &self.equalities {
            if data[id1] != data[id2] {
                return false;
            }
        }
        self.pattern
            .iter()
            .copied()
            .zip(data.iter().copied())
            .all(|(pat, sym)| pat < 0 || (pat as u32) == sym)
    }
    fn bind(&self, row: &[Sym], out: &mut [u32]) {
        debug_assert!(self.matches(row));
        for (i, pat) in self.pattern.iter().copied().enumerate() {
            if let Some(var) = Self::as_var(pat) {
                out[var as usize] = row[i];
            }
        }
    }
}

impl<T: AsRef<[Arg]>> From<T> for Pattern {
    fn from(value: T) -> Self {
        Pattern::new(value)
    }
}

struct PatternN<'pat, const N: usize> {
    pattern: &'pat [i32; N],
    equalities: &'pat [[usize; 2]],
    // smallest/biggest rows that may match this pattern
    // These bounds will be tight if the variables are all at the end of the pattern.
    // In that case, the symbols at the start can be used as an index into the table.
    min: [Sym; N],
    max: [Sym; N],
}
impl<'pat, const N: usize> PatternN<'pat, N> {
    /// Returns true if the pattern matches the row.
    pub fn matches(&self, data: &[Sym; N]) -> bool {
        for &[id1, id2] in self.equalities {
            if data[id1] != data[id2] {
                return false;
            }
        }
        self.pattern
            .iter()
            .copied()
            .zip_eq(data.iter().copied())
            .all(|(pat, sym)| pat < 0 || (pat as u32) == sym)
    }

    pub fn find_matches<'me>(&'me self, dataset: &'me [[Sym; N]]) -> impl Iterator<Item = &'me [Sym; N]> + 'me {
        // drop all elements that are smaller than the smallest match
        let first_possible_match = dataset.partition_point(|row| row < &self.min);
        let dataset = &dataset[first_possible_match..];
        // drop all elements that are bigger that the biggest match
        let after_last_possible_match = dataset.partition_point(|row| row <= &self.max);
        let dataset = &dataset[..after_last_possible_match];
        // match all remaining elements
        dataset.iter().filter(|row| self.matches(row))
    }

    pub fn bind(&self, row: &Fact<N>, out: &mut [u32]) {
        debug_assert!(self.matches(row));
        for (i, pat) in self.pattern.iter().copied().enumerate() {
            if let Some(var) = Pattern::as_var(pat) {
                out[var as usize] = row[i];
            }
        }
    }

    /// Specialized version of [`Pattern::all_bindings`]
    pub fn all_bindings<'a>(&'a self, table: &'a [[Sym; N]], out: &'a [u32]) -> TableBuff<u32> {
        let mut bindings = TableBuff::new(out.len());
        let mut out_row = Vec::from(out);
        for row in self.find_matches(table) {
            self.bind(row, &mut out_row);
            bindings.push(&out_row);
        }
        bindings
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_pattern_spec() {
        let mut table = [
            [1, 2, 1],
            [1, 2, 2],
            [1, 2, 3],
            [1, 2, 4],
            [1, 2, 5],
            [2, 2, 1],
            [2, 2, 2],
            [2, 2, 3],
            [2, 2, 4],
            [2, 2, 5],
            [2, 2, 6],
            [2, 2, 7],
            [1, 3, 1],
            [1, 3, 2],
            [1, 3, 3],
            [1, 3, 4],
            [1, 3, 5],
            [1, 3, 6],
        ];
        table.sort();

        let check_matches = |pattern: [Arg; 3], expected: &[Fact<3>]| {
            let pattern = Pattern::new(pattern.as_slice());
            let pattern = pattern.compiled();
            let matches = pattern.find_matches(&table).cloned().sorted().collect_vec();
            let expected = expected.iter().cloned().sorted().collect_vec();
            assert_eq!(matches, expected)
        };

        use Arg::*;

        check_matches([Var(0), Var(1), Sym(3)], &[[1, 2, 3], [2, 2, 3], [1, 3, 3]]);

        check_matches([Var(0), Var(0), Sym(3)], &[[2, 2, 3]]);
        check_matches([Var(0), Var(1), Var(0)], &[[1, 2, 1], [1, 3, 1], [2, 2, 2]]);
        check_matches([Sym(1), Sym(3), Sym(6)], &[[1, 3, 6]]);
        check_matches([Sym(1), Sym(3), Sym(0)], &[]);

        // let pattern = Pattern::new([Var(0), Sym(1), Var(0)]);
        // let bindings: &mut [u32] = &mut [0; 2];
    }

    #[test]
    fn test_pattern_generic() {
        let table = Table::new_from_flat(
            6,
            vec![
                1, 2, 3, 10, 5, 20, 2, 2, 3, 11, 5, 21, 1, 3, 3, 12, 5, 22, 2, 2, 3, 30, 5, 30, 4, 4, 3, 40, 5, 40, 1,
                5, 1, 50, 5, 50, 2, 6, 2, 60, 5, 60, 1, 3, 6, 70, 5, 70,
            ],
        );

        let check_matches = |pattern: &[Arg], expected: &[&[_]]| {
            let pattern = Pattern::new(pattern);
            let num_vars = pattern.vars().unique().count();
            let empty_bindings = vec![super::Sym::MAX; num_vars];
            let bindings = pattern.all_bindings(&table, &empty_bindings);
            let mut results: Vec<Vec<_>> = bindings.rows().map(|r| r.to_vec()).collect();
            results.sort();
            let mut expected: Vec<Vec<_>> = expected.iter().map(|e| e.to_vec()).collect();
            expected.sort();
            assert_eq!(results, expected)
        };

        use Arg::*;

        check_matches(
            &[Var(0), Var(1), Sym(3), Var(2), Sym(5), Var(3)],
            &[
                &[1, 2, 10, 20],
                &[2, 2, 11, 21],
                &[1, 3, 12, 22],
                &[2, 2, 30, 30],
                &[4, 4, 40, 40],
            ],
        );

        check_matches(&[Var(0), Var(0), Sym(3), Var(1), Sym(5), Var(1)], &[&[2, 30], &[4, 40]]);
        check_matches(
            &[Var(0), Var(1), Var(0), Var(2), Sym(5), Var(2)],
            &[&[1, 5, 50], &[2, 6, 60]],
        );
        check_matches(&[Sym(1), Sym(3), Sym(6), Var(0), Sym(5), Var(0)], &[&[70]]);
        check_matches(&[Sym(1), Sym(3), Sym(0), Var(0), Sym(5), Var(0)], &[]);
    }

    #[test]
    fn test_join() {
        // path(x, y) :- edge(x, z), path(z, y)
        let join = Join::new(
            2, // only select the first two variables
            Pattern::new([Arg::Var(0), Arg::Var(2)]),
            Pattern::new([Arg::Var(2), Arg::Var(1)]),
        );

        let edge_var = VarTable::from([[1, 2], [1, 3], [2, 4]]);
        let path_var = VarTable::from([[4, 5], [5, 6]]);
        for _ in 0..5 {
            join.run(
                &edge_var.recent.borrow(),
                &path_var.recent.borrow(),
                &mut path_var.to_add.borrow_mut(),
            );
            join.run(
                &edge_var.stable.borrow(),
                &path_var.recent.borrow(),
                &mut path_var.to_add.borrow_mut(),
            );
            join.run(
                &edge_var.recent.borrow(),
                &path_var.stable.borrow(),
                &mut path_var.to_add.borrow_mut(),
            );

            edge_var.process();
            path_var.process();

            println!("\nFinal:");
            for row in path_var.stable.borrow().rows() {
                println!("  {row:?}");
            }
        }

        // println!("\nFinal:");
        // for row in path_var.stable.borrow().rows() {
        //     println!("  {row:?}");
        // }
    }
}
