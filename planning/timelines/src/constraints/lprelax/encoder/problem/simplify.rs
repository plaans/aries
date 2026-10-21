//! The simplification of an [`LpRelaxProblem`].

use hashbrown::HashMap;

use aries_linexpr::ComparisonOp;
use aries_solver::core::IntCst;

use crate::lprelax::encoder::LpRelaxEncoder;
use crate::{Domains, SchedEncoder};

use super::{ColTag, LinExpr, LpRelaxProblem, RowExpr};

/// The problem was found trivially infeasible during simplification.
type Infeasible = ();

pub(super) fn try_simplify(
    pb: &mut LpRelaxProblem,
    encoder: &LpRelaxEncoder,
    ctx: &SchedEncoder,
    doms: &Domains,
    merge_equal_columns: bool,
) -> Result<(), Infeasible> {
    try_simplify_inner(pb, &mut EquivClasses::new(merge_equal_columns), encoder, ctx, doms)
}

fn try_simplify_inner(
    pb: &mut LpRelaxProblem,
    classes: &mut EquivClasses,
    encoder: &LpRelaxEncoder,
    ctx: &SchedEncoder,
    doms: &Domains,
) -> Result<(), Infeasible> {
    let mut times = vec![];

    let time = std::time::Instant::now();
    seed_known_values(classes, encoder, ctx, doms)?;
    times.push(time.elapsed());

    // The columns are interned once: the rounds below only deal with their classes.
    let time = std::time::Instant::now();
    let rows = pb
        .rows
        .iter()
        .map(|row| ClassRow::interned(row, classes))
        .collect::<Vec<_>>();
    times.push(time.elapsed());

    // Learn from the rows until nothing new comes out.
    // A row whose columns all have a known value is done for good, and left out of the next rounds.
    let time = std::time::Instant::now();
    let mut rounds = 0;
    let mut live = (0..rows.len()).collect::<Vec<_>>();
    let canons = loop {
        rounds += 1;
        let mut learned = false;
        let mut canons = Vec::with_capacity(live.len());
        for &i in &live {
            let canon = rows[i].canonical(classes);
            learned |= canon.learn(classes)?;
            if !canon.expr.terms().is_empty() {
                canons.push((i, canon));
            }
        }
        if !learned {
            break canons;
        }
        live = canons.into_iter().map(|(i, _)| i).collect();
    };
    times.push(time.elapsed());

    // Nothing was learned in the last round, the canonical rows from last round are final.
    let time = std::time::Instant::now();
    pb.rows = canons
        .into_iter()
        .map(|(_, canon)| {
            canon
                .expr
                .iter()
                .map(|(class, coef)| (classes.representative(class), coef))
                .collect::<LinExpr>()
                .into_constraint(canon.tpe, canon.cst())
        })
        .collect();
    times.push(time.elapsed());

    // A tag resolves to the column of its class' representative, or to its known value.
    let time = std::time::Instant::now();
    let mut aliases = vec![];
    pb.col_known_value.clear();
    for (tag, class) in classes.interned() {
        match classes.value(class) {
            Some(value) => {
                pb.col_known_value.insert(tag, value);
            }
            None => aliases.push((tag, classes.representative(class))),
        }
    }
    pb.number_cols(aliases);
    times.push(time.elapsed());

    lprelax_log!(
        "LpRelaxProblem simplified in {} (seeding {}, interning {}, {} rounds {}, rewriting {}, numbering columns {})",
        times.iter().sum::<std::time::Duration>().as_secs_f64(),
        times[0].as_secs_f64(),
        times[1].as_secs_f64(),
        rounds,
        times[2].as_secs_f64(),
        times[3].as_secs_f64(),
        times[4].as_secs_f64(),
    );

    Ok(())
}

/// Records the values that the bindings to the main model already fix, given the domains (see [`LpRelaxEncoder::bindings`]).
fn seed_known_values(
    classes: &mut EquivClasses,
    encoder: &LpRelaxEncoder,
    ctx: &SchedEncoder,
    doms: &Domains,
) -> Result<(), Infeasible> {
    for binding in encoder.bindings(ctx) {
        if binding.trigger.iter().all(|&lit| doms.entails(lit)) {
            let class = classes.intern(binding.tag);
            classes.set_value(class, binding.value)?;
        }
    }
    Ok(())
}

/// Equivalence classes of columns (a union-find over the interned tags), with their known values and representatives.
/// When `merging` is disabled, each class stays a singleton.
struct EquivClasses {
    merging: bool,
    tags: Vec<ColTag>,
    index: HashMap<ColTag, u32>,
    parent: Vec<u32>,
    size: Vec<u32>,
    /// For each class (by root, i.e. at the index given by [`Self::find`]), the index of the tag it is represented by.
    representative: Vec<u32>,
    /// For each class (by root, i.e. at the index given by [`Self::find`]), its value once known.
    values: Vec<Option<IntCst>>,
}
impl EquivClasses {
    fn new(merging: bool) -> Self {
        Self {
            merging,
            tags: vec![],
            index: HashMap::new(),
            parent: vec![],
            size: vec![],
            representative: vec![],
            values: vec![],
        }
    }

    fn intern(&mut self, tag: ColTag) -> u32 {
        if let Some(&i) = self.index.get(&tag) {
            return i;
        }
        let i = self.tags.len() as u32;
        self.tags.push(tag);
        self.parent.push(i);
        self.size.push(1);
        self.representative.push(i);
        self.values.push(None);
        self.index.insert(tag, i);
        i
    }

    /// All interned tags, with their index.
    fn interned(&self) -> Vec<(ColTag, u32)> {
        self.index.iter().map(|(&tag, &i)| (tag, i)).collect()
    }

    fn find(&mut self, mut i: u32) -> u32 {
        while self.parent[i as usize] != i {
            let grandparent = self.parent[self.parent[i as usize] as usize];
            self.parent[i as usize] = grandparent;
            i = grandparent;
        }
        i
    }

    fn value(&mut self, i: u32) -> Option<IntCst> {
        let root = self.find(i);
        self.values[root as usize]
    }

    /// The tag that the class of `i` is represented by (a lifted one whenever the class holds one).
    fn representative(&mut self, i: u32) -> ColTag {
        let root = self.find(i);
        self.tags[self.representative[root as usize] as usize]
    }

    /// Records that the class of `i` takes value `v`. Returns whether that was new.
    fn set_value(&mut self, i: u32, v: IntCst) -> Result<bool, Infeasible> {
        if v < 0 || self.representative(i).upper_bound().is_some_and(|ub| v > ub) {
            return Err(()); // out of the column's range
        }
        let root = self.find(i);
        match self.values[root as usize] {
            Some(old) if old != v => Err(()), // leaves the known value in place, as `merge` does
            Some(_) => Ok(false),
            None => {
                self.values[root as usize] = Some(v);
                Ok(true)
            }
        }
    }

    /// Merges the classes of `a` and `b` (when merging is enabled). Returns whether that was new.
    fn merge(&mut self, a: u32, b: u32) -> Result<bool, Infeasible> {
        if !self.merging {
            return Ok(false);
        }
        let (mut kept, mut merged) = (self.find(a), self.find(b));
        if kept == merged {
            return Ok(false);
        }
        let value = match (self.values[kept as usize], self.values[merged as usize]) {
            (Some(va), Some(vb)) if va != vb => return Err(()),
            (va, vb) => va.or(vb),
        };

        // Union by size.
        if self.size[kept as usize] < self.size[merged as usize] {
            std::mem::swap(&mut kept, &mut merged);
        }
        self.parent[merged as usize] = kept;
        self.size[kept as usize] += self.size[merged as usize];
        self.values[kept as usize] = value;

        // Prefer a lifted source presence, then any lifted tag (ties broken by the tags' order).
        let key = |i: u32| {
            let tag = self.tags[i as usize];
            (
                !matches!(tag, ColTag::PresenceSource(_, None)),
                !tag.is_lifted_or_term_grounding(),
                tag,
            )
        };
        let (r_kept, r_merged) = (self.representative[kept as usize], self.representative[merged as usize]);
        self.representative[kept as usize] = if key(r_kept) <= key(r_merged) { r_kept } else { r_merged };

        Ok(true)
    }
}

/// A row over the classes of its columns, held as `expr cmp 0`.
struct ClassRow {
    tpe: ComparisonOp,
    expr: LinExpr<u32>,
}
impl ClassRow {
    fn interned(row: &RowExpr, classes: &mut EquivClasses) -> Self {
        let row = row.canonical();
        let mut expr = LinExpr::from_constant(row.expr().constant());
        for (col_tag, coef) in row.expr().iter() {
            expr.push((classes.intern(col_tag), coef).into());
        }
        Self {
            tpe: row.comp_op(),
            expr,
        }
    }

    /// The row over the representatives of its classes, with the known values moved into the constant.
    fn canonical(&self, classes: &mut EquivClasses) -> Self {
        let mut expr = LinExpr::from_constant(self.expr.constant());
        for (class, coef) in self.expr.iter() {
            let root = classes.find(class);
            match classes.value(root) {
                Some(value) => expr += coef * value,
                None => expr.push((root, coef).into()),
            }
        }
        expr.simplify();
        Self { tpe: self.tpe, expr }
    }

    /// The constant `cst` of the row, read as `sum cmp cst`.
    fn cst(&self) -> IntCst {
        -self.expr.constant()
    }

    /// Learns what this (canonical) row implies on its own. Returns whether anything was learned.
    fn learn(&self, classes: &mut EquivClasses) -> Result<bool, Infeasible> {
        let cst = self.cst();

        // Range `[min, max]` of the row's sum (`None` for an unbounded end).
        let (mut min, mut max) = (Some(0), Some(0));
        for (class, coef) in self.expr.iter() {
            let end = classes.representative(class).upper_bound().map(|ub| coef * ub);
            if coef > 0 {
                max = max.zip(end).map(|(max, end)| max + end);
            } else {
                min = min.zip(end).map(|(min, end)| min + end);
            }
        }

        let (lb, ub) = match self.tpe {
            ComparisonOp::Eq => (Some(cst), Some(cst)),
            ComparisonOp::Le => (None, Some(cst)),
            ComparisonOp::Ge => (Some(cst), None),
        };
        if lb.zip(max).is_some_and(|(lb, max)| lb > max) || ub.zip(min).is_some_and(|(ub, min)| ub < min) {
            return Err(());
        }

        let mut learned = false;

        // At one end of its range, the sum pins each of its columns to their corresponding ends.
        if lb.is_some() && lb == max {
            for (class, coef) in self.expr.iter() {
                let col_ub = classes.representative(class).upper_bound();
                learned |= classes.set_value(class, if coef > 0 { col_ub.unwrap() } else { 0 })?;
            }
        }
        if ub.is_some() && ub == min {
            for (class, coef) in self.expr.iter() {
                let col_ub = classes.representative(class).upper_bound();
                learned |= classes.set_value(class, if coef > 0 { 0 } else { col_ub.unwrap() })?;
            }
        }

        // `c*x - c*y = 0` proves the two columns equal.
        if self.tpe == ComparisonOp::Eq
            && cst == 0
            && let [a, b] = self.expr.terms()
            && a.coeff == -b.coeff
        {
            learned |= classes.merge(a.var, b.var)?;
        }

        Ok(learned)
    }
}

#[cfg(test)]
mod tests {
    use aries_linexpr::IntoLinExpr;

    use super::*;

    fn prez_trans_lifted(trans: usize) -> ColTag {
        ColTag::PresenceTransition(trans, None)
    }
    fn prez_source_ground(grounding: usize) -> ColTag {
        ColTag::PresenceSource(None, Some(grounding))
    }
    /// A row `sum(terms) cmp cst`.
    fn row(tpe: ComparisonOp, terms: Vec<(IntCst, ColTag)>, cst: IntCst) -> RowExpr {
        terms
            .into_iter()
            .map(|(coef, tag)| (tag, coef))
            .collect::<LinExpr>()
            .into_constraint(tpe, cst)
    }
    fn canonical(row: &RowExpr, classes: &mut EquivClasses) -> ClassRow {
        ClassRow::interned(row, classes).canonical(classes)
    }
    /// The coefficient of the class of `tag` in `canon` (0 if absent).
    fn coef_of(canon: &ClassRow, classes: &mut EquivClasses, tag: ColTag) -> IntCst {
        let class = classes.intern(tag);
        let root = classes.find(class);
        canon.expr.iter().find(|&(c, _)| c == root).map_or(0, |(_, coef)| coef)
    }

    /// A class holds at most one value, and keeps it when merged with another class.
    #[test]
    fn classes_track_values_and_reject_conflicts() {
        let mut classes = EquivClasses::new(true);
        let a = classes.intern(prez_trans_lifted(0));
        let b = classes.intern(prez_trans_lifted(1));

        assert_eq!(classes.intern(prez_trans_lifted(0)), a);
        assert_eq!(classes.value(a), None);

        assert!(classes.set_value(a, 1).unwrap()); // new
        assert!(!classes.set_value(a, 1).unwrap()); // already known
        assert!(classes.set_value(a, 0).is_err()); // conflict

        assert!(classes.merge(a, b).unwrap());
        assert_eq!(classes.value(b), Some(1));

        let mut classes = EquivClasses::new(true);
        let a = classes.intern(prez_trans_lifted(0));
        let b = classes.intern(prez_trans_lifted(1));
        classes.set_value(a, 0).unwrap();
        classes.set_value(b, 1).unwrap();
        assert!(classes.merge(a, b).is_err());
    }

    /// A class is represented by a lifted tag whenever it holds one, and is never merged when merging is disabled.
    #[test]
    fn merging_prefers_a_lifted_representative() {
        let mut classes = EquivClasses::new(true);
        let g = classes.intern(prez_source_ground(0));
        let l = classes.intern(prez_trans_lifted(7));

        assert!(classes.merge(g, l).unwrap());
        assert_eq!(classes.find(g), classes.find(l));
        assert_eq!(classes.representative(g), prez_trans_lifted(7));
        assert!(!classes.merge(g, l).unwrap()); // already merged

        let mut classes = EquivClasses::new(false);
        let g = classes.intern(prez_source_ground(0));
        let l = classes.intern(prez_trans_lifted(7));

        assert!(!classes.merge(g, l).unwrap());
        assert_ne!(classes.find(g), classes.find(l));
    }

    /// `lhs cmp rhs` reads as `lhs - rhs cmp cst`, with the known values moved into `cst`.
    #[test]
    fn canonicalisation_negates_the_rhs_and_moves_known_values_into_cst() {
        let mut classes = EquivClasses::new(true);
        // `x = y + 1`, i.e. `x - y = 1`
        let r = prez_trans_lifted(0).into_linexpr().eq(prez_trans_lifted(1) + 1);

        let canon = canonical(&r, &mut classes);
        assert_eq!(canon.cst(), 1);
        assert_eq!(coef_of(&canon, &mut classes, prez_trans_lifted(0)), 1);
        assert_eq!(coef_of(&canon, &mut classes, prez_trans_lifted(1)), -1);

        // with `y = 1`, the row reads `x = 2`
        let y = classes.intern(prez_trans_lifted(1));
        classes.set_value(y, 1).unwrap();

        let canon = canonical(&r, &mut classes);
        assert_eq!(canon.cst(), 2);
        assert_eq!(coef_of(&canon, &mut classes, prez_trans_lifted(0)), 1);
        assert_eq!(coef_of(&canon, &mut classes, prez_trans_lifted(1)), 0);
    }

    /// An equality row at one end of its range pins its columns, and `x - y = 0` merges them.
    #[test]
    fn an_equality_row_pins_or_merges_its_columns() {
        // `x + y = 2` gives `x = y = 1`, and `x + y = 0` gives `x = y = 0`
        for (cst, pinned_to) in [(2, 1), (0, 0)] {
            let mut classes = EquivClasses::new(true);
            let r = row(
                ComparisonOp::Eq,
                vec![(1, prez_trans_lifted(0)), (1, prez_trans_lifted(1))],
                cst,
            );
            assert!(canonical(&r, &mut classes).learn(&mut classes).unwrap());

            for tag in [prez_trans_lifted(0), prez_trans_lifted(1)] {
                let class = classes.intern(tag);
                assert_eq!(classes.value(class), Some(pinned_to));
            }
        }

        let mut classes = EquivClasses::new(true);
        let r = row(
            ComparisonOp::Eq,
            vec![(1, prez_trans_lifted(0)), (-1, prez_trans_lifted(1))],
            0,
        );
        assert!(canonical(&r, &mut classes).learn(&mut classes).unwrap());

        let (x, y) = (
            classes.intern(prez_trans_lifted(0)),
            classes.intern(prez_trans_lifted(1)),
        );
        assert_eq!(classes.find(x), classes.find(y));
        assert_eq!(classes.value(x), None);
    }

    /// A row out of reach of its columns is infeasible, before or after moving the known values into the constant.
    #[test]
    fn a_row_out_of_the_reach_of_its_columns_is_infeasible() {
        let mut classes = EquivClasses::new(true);
        // `x >= 2` with `x` in `[0, 1]`
        let r = row(ComparisonOp::Ge, vec![(1, prez_trans_lifted(0))], 2);
        assert!(canonical(&r, &mut classes).learn(&mut classes).is_err());

        let mut classes = EquivClasses::new(true);
        let x = classes.intern(prez_trans_lifted(0));
        classes.set_value(x, 0).unwrap();
        // `x >= 1` with `x = 0`, i.e. `0 >= 1`
        let r = row(ComparisonOp::Ge, vec![(1, prez_trans_lifted(0))], 1);
        assert!(canonical(&r, &mut classes).learn(&mut classes).is_err());
    }
}
