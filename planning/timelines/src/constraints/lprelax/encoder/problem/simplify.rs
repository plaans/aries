//! The simplification pass over an [`LpRelaxProblem`], and the equivalence classes it works on.

use hashbrown::HashMap;

use aries_solver::core::IntCst;

use crate::{Domains, SchedEncoder};

use super::super::LpRelaxEncoder;
use super::{ColTag, LpRelaxProblem, RowExpr, RowExprType};

/// Records the values that the domains already fix for the lifted presence and support columns,
/// and for the term grounding columns appearing in the rows.
fn seed_known_values(
    pb: &LpRelaxProblem,
    classes: &mut EquivClasses,
    encoder: &LpRelaxEncoder,
    ctx: &SchedEncoder,
    doms: &Domains,
) -> Result<(), Infeasible> {
    let lifted_presence_and_support_cols = encoder
        .iter_sources()
        .flat_map(|(source, trans_ids)| {
            std::iter::chain(
                [(
                    ColTag::PresenceSource(source, None),
                    encoder.get_source_prez(source, ctx),
                )],
                trans_ids.iter().map(|&trans_id| {
                    (
                        ColTag::PresenceTransition(trans_id, None),
                        encoder.transitions.get_prez(trans_id, ctx),
                    )
                }),
            )
        })
        .chain(
            encoder
                .supports_sorted
                .as_ref()
                .unwrap()
                .sorted_out()
                .iter()
                .filter_map(|&((out_trans_id, in_trans_id), active)| {
                    active.map(|active| (ColTag::Support(out_trans_id, in_trans_id, None), active))
                }),
        );

    for (col_tag, lit) in lifted_presence_and_support_cols {
        let value = if doms.entails(doms.presence(lit)) {
            match doms.value(lit) {
                Some(true) => 1,
                Some(false) => 0,
                None => continue,
            }
        } else if doms.entails(!doms.presence(lit)) {
            0
        } else {
            continue;
        };

        if value == 1 && matches!(col_tag, ColTag::Support(..)) && encoder.supports.with_condition_out_transitions {
            // In the case where conditions can be used as out-transitions,
            // it is UNSOUND to derive all the columns corresponding to present and true causal link support literals as equal to 1.
            // Indeed, that case forbids the LP relaxation from having an effect support 2 or more conditions, even though it is allowed in the main model.
            // As such, this could force two support columns two 1, making their sum equal to 2,
            // while this very sum would be constrained to be <= 1 by the lp relaxation (with conditions allowed to be out-transitions),
            // which was contradictory.
            continue;
        }

        let class = classes.intern(col_tag);
        classes.set_value(class, value)?;
    }

    for row in &pb.rows {
        for &(_, col_tag) in &row.terms {
            let ColTag::TermGround(term, value) = col_tag else {
                continue;
            };

            // Out of the term's bounds: the column is 0 whether or not the term is present.
            // (It is 1 only if the term is known present and fixed to that value).
            let known = if doms.entails(!doms.presence(term)) || value < doms.lb(term) || doms.ub(term) < value {
                0
            } else if doms.entails(doms.presence(term)) && doms.lb(term) == doms.ub(term) {
                1
            } else {
                continue;
            };

            let class = classes.intern(col_tag);
            classes.set_value(class, known)?;
        }
    }

    Ok(())
}

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
    let total_time = std::time::Instant::now();

    let time = std::time::Instant::now();
    seed_known_values(pb, classes, encoder, ctx, doms)?;
    let seeding_time = time.elapsed();

    // The rows' columns are interned once and for all: the rounds below only deal with their classes.
    let time = std::time::Instant::now();
    let rows = pb
        .rows
        .iter()
        .map(|row| InternedRow::of(row, classes))
        .collect::<Vec<_>>();
    let interning_time = time.elapsed();

    let num_terms: usize = rows.iter().map(|row| row.terms.len()).sum();
    let sum_squared_row_lens: usize = rows.iter().map(|row| row.terms.len().pow(2)).sum();
    let max_row_len = rows.iter().map(|row| row.terms.len()).max().unwrap_or(0);

    // Learn values (and equalities) from the rows until nothing new comes out.
    // Each round re-reads the rows, whose canonical form only gets simpler as knowledge grows.
    // A row whose columns all have a known value is done for good (`learn` has checked its constant), and is left out of the next rounds.
    let time = std::time::Instant::now();
    let mut rounds = 0;
    let mut live = (0..rows.len()).collect::<Vec<_>>();
    let canons = loop {
        rounds += 1;
        let mut learned = false;
        let mut canons = Vec::with_capacity(live.len());
        for &i in &live {
            let canon = CanonicalRow::of(&rows[i], classes);
            learned |= canon.learn(classes)?;
            if !canon.coefs.is_empty() {
                canons.push((i, canon));
            }
        }
        if !learned {
            break canons;
        }
        live = canons.into_iter().map(|(i, _)| i).collect();
    };
    let rounds_time = time.elapsed();

    // Rewrite the rows over the representatives of the columns that are left, from the canonical forms of the last round:
    // nothing was learned in that round, so they are final.
    let time = std::time::Instant::now();
    pb.rows = canons
        .into_iter()
        .map(|(_, canon)| {
            let terms = canon
                .coefs
                .iter()
                .map(|&(coef, class)| (coef, classes.representative(class)))
                .collect::<Vec<_>>();
            let separator = terms.len();
            RowExpr::new(canon.tpe, terms, separator, canon.cst)
        })
        .collect();
    let rewriting_time = time.elapsed();

    // Every "surviving" column tag resolves to its equivalence class' representative column.
    // Tags of classes with a known value aren't columns anymore: their value is recorded instead.
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
    let numbering_time = time.elapsed();

    tracing::info!(
        "|-[LPRELAX]---- simplification phases: total {}s = seeding {}s, interning {}s, {} rounds {}s, rewriting {}s, numbering columns {}s ({} terms, max row length {}, sum of squared row lengths {})",
        total_time.elapsed().as_secs_f64(),
        seeding_time.as_secs_f64(),
        interning_time.as_secs_f64(),
        rounds,
        rounds_time.as_secs_f64(),
        rewriting_time.as_secs_f64(),
        numbering_time.as_secs_f64(),
        num_terms,
        max_row_len,
        sum_squared_row_lens,
    );

    Ok(())
}

/// The problem was found trivially infeasible during simplification.
type Infeasible = ();

/// Equivalence classes over the columns, together with the value / representative of a class (once it is known).
///
/// (Only used / merged when `merging` is enabled -- otherwise each class stays a singleton).
struct EquivClasses {
    merging: bool,
    tags: Vec<ColTag>,
    index: HashMap<ColTag, u32>,
    /// Union-find over the indices in `tags`.
    parent: Vec<u32>,
    size: Vec<u32>,
    /// For each class (by root), the index of the tag it is represented by.
    representative: Vec<u32>,
    /// For each class (by root), its value once known.
    values: Vec<Option<IntCst>>,
}
impl EquivClasses {
    pub(super) fn new(merging: bool) -> Self {
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

    /// All interned tags, with the index they were interned at.
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
        if !(0..=1).contains(&v) {
            return Err(()); // columns live in `[0, 1]`
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

    /// Merges the classes of `a` and `b`, some row having proven them equal.
    /// Returns whether that was new.
    ///
    /// Does nothing when merging is disabled.
    fn merge(&mut self, a: u32, b: u32) -> Result<bool, Infeasible> {
        if !self.merging {
            return Ok(false);
        }

        let (mut kept, mut merged) = (self.find(a), self.find(b));
        if kept == merged {
            return Ok(false);
        }
        if let (Some(va), Some(vb)) = (self.values[kept as usize], self.values[merged as usize])
            && va != vb
        {
            return Err(());
        }

        if self.size[kept as usize] < self.size[merged as usize] {
            std::mem::swap(&mut kept, &mut merged);
        }
        self.parent[merged as usize] = kept;
        self.size[kept as usize] += self.size[merged as usize];

        if let Some(v) = self.values[merged as usize].take() {
            self.values[kept as usize] = Some(v);
        }

        // Of the two representatives, prefer a lifted tag; break ties deterministically.
        let (r_kept, r_merged) = (self.representative[kept as usize], self.representative[merged as usize]);
        let key = |i: u32| {
            let tag = self.tags[i as usize];
            (!tag.is_lifted_or_term_grounding(), tag)
        };
        self.representative[kept as usize] = if key(r_kept) <= key(r_merged) { r_kept } else { r_merged };

        Ok(true)
    }
}

/// A row with its columns interned: `sum(coef * class) cmp cst`, the rhs terms being negated.
struct InternedRow {
    tpe: RowExprType,
    terms: Vec<(IntCst, u32)>,
    cst: IntCst,
}
impl InternedRow {
    fn of(row: &RowExpr, classes: &mut EquivClasses) -> Self {
        let terms = row
            .terms
            .iter()
            .enumerate()
            .map(|(i, &(coef, col_tag))| {
                // `lhs cmp rhs + cst` reads as `sum(lhs) - sum(rhs) cmp cst`.
                let coef = if i < row.separator { coef } else { -coef };
                (coef, classes.intern(col_tag))
            })
            .collect();

        Self {
            tpe: row.tpe,
            terms,
            cst: row.cst(),
        }
    }
}

/// A row rewritten with the representative columns / tags of the union-find
struct CanonicalRow {
    tpe: RowExprType,
    /// Sorted by class, without duplicates.
    coefs: Vec<(IntCst, u32)>,
    cst: IntCst,
}
impl CanonicalRow {
    fn of(row: &InternedRow, classes: &mut EquivClasses) -> Self {
        let mut coefs: Vec<(IntCst, u32)> = Vec::with_capacity(row.terms.len());
        let mut cst = row.cst;

        for &(coef, class) in &row.terms {
            let root = classes.find(class);
            match classes.value(root) {
                Some(value) => cst -= coef * value,
                None => coefs.push((coef, root)),
            }
        }

        // Sum up the coefficients of a same class, then drop those that cancel out.
        coefs.sort_unstable_by_key(|&(_, root)| root);
        coefs.dedup_by(|next, prev| {
            if next.1 == prev.1 {
                prev.0 += next.0;
                true
            } else {
                false
            }
        });
        coefs.retain(|&(coef, _)| coef != 0);

        Self {
            tpe: row.tpe,
            coefs,
            cst,
        }
    }

    /// Applies everything this row implies on its own.
    /// Returns whether anything was learned.
    fn learn(&self, classes: &mut EquivClasses) -> Result<bool, Infeasible> {
        // The row reads `sum cmp cst`, where `sum` ranges over `[min, max]` given `[0, 1]` columns.
        let min: IntCst = self.coefs.iter().map(|&(coef, _)| coef.min(0)).sum();
        let max: IntCst = self.coefs.iter().map(|&(coef, _)| coef.max(0)).sum();

        let (lb, ub) = match self.tpe {
            RowExprType::Eq => (Some(self.cst), Some(self.cst)),
            RowExprType::Leq => (None, Some(self.cst)),
            RowExprType::Geq => (Some(self.cst), None),
        };

        if lb.is_some_and(|lb| lb > max) || ub.is_some_and(|ub| ub < min) {
            return Err(());
        }

        let mut learned = false;

        // At one of its bounds, every column of the row is pinned to the end of its own range.
        if lb == Some(max) {
            for &(coef, class) in &self.coefs {
                learned |= classes.set_value(class, if coef > 0 { 1 } else { 0 })?;
            }
        }
        if ub == Some(min) {
            for &(coef, class) in &self.coefs {
                learned |= classes.set_value(class, if coef > 0 { 0 } else { 1 })?;
            }
        }

        // `c*x - c*y = 0` proves the two columns equal.
        if self.tpe == RowExprType::Eq
            && self.cst == 0
            && let [(coef_a, a), (coef_b, b)] = self.coefs[..]
            && coef_a == -coef_b
        {
            learned |= classes.merge(a, b)?;
        }

        Ok(learned)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prez_trans_lifted(trans: usize) -> ColTag {
        ColTag::PresenceTransition(trans, None)
    }
    fn prez_source_ground(grounding: usize) -> ColTag {
        ColTag::PresenceSource(None, Some(grounding))
    }
    /// A row `sum(terms) cmp cst`, all terms on the lhs.
    fn row(tpe: RowExprType, terms: Vec<(IntCst, ColTag)>, cst: IntCst) -> RowExpr {
        let separator = terms.len();
        RowExpr::new(tpe, terms, separator, cst)
    }
    /// The canonical form of `row` (given what `classes` currently know).
    fn canonical(row: &RowExpr, classes: &mut EquivClasses) -> CanonicalRow {
        let row = InternedRow::of(row, classes);
        CanonicalRow::of(&row, classes)
    }
    /// The coefficient that `canon` carries for the class of `tag` (0 if it carries none).
    fn coef_of(canon: &CanonicalRow, classes: &mut EquivClasses, tag: ColTag) -> IntCst {
        let class = classes.intern(tag);
        let root = classes.find(class);
        canon
            .coefs
            .iter()
            .find(|&&(_, c)| c == root)
            .map_or(0, |&(coef, _)| coef)
    }

    /// An equivalence class holds at most one value, within `[0, 1]`, and a union carries it over.
    #[test]
    fn classes_track_values_and_reject_conflicts() {
        let mut classes = EquivClasses::new(true);
        let a = classes.intern(prez_trans_lifted(0));
        let b = classes.intern(prez_trans_lifted(1));

        assert_eq!(
            classes.intern(prez_trans_lifted(0)),
            a,
            "interning a tag again yields its class"
        );
        assert_eq!(classes.value(a), None);

        assert!(classes.set_value(a, 1).unwrap()); // learned something new
        assert!(!classes.set_value(a, 1).unwrap()); // already known
        assert!(classes.set_value(a, 0).is_err()); // contradicts what is known
        assert!(classes.set_value(b, 2).is_err()); // a column lives in [0, 1]

        assert!(classes.merge(a, b).unwrap());
        assert_eq!(classes.value(b), Some(1)); // the union carries the value over

        let mut classes = EquivClasses::new(true);
        let a = classes.intern(prez_trans_lifted(0));
        let b = classes.intern(prez_trans_lifted(1));
        classes.set_value(a, 0).unwrap();
        classes.set_value(b, 1).unwrap();
        assert!(classes.merge(a, b).is_err()); // cannot be equal and differ
    }

    /// A class is represented by a lifted tag whenever it holds one,
    /// and is never merged at all when merging is disabled.
    #[test]
    fn merging_prefers_a_lifted_representative() {
        let mut classes = EquivClasses::new(true);
        let g = classes.intern(prez_source_ground(0));
        let l = classes.intern(prez_trans_lifted(7));

        assert!(classes.merge(g, l).unwrap());
        assert_eq!(classes.find(g), classes.find(l));
        assert_eq!(classes.representative(g), prez_trans_lifted(7));
        assert_eq!(classes.representative(l), prez_trans_lifted(7));
        assert!(!classes.merge(g, l).unwrap()); // already merged

        let mut classes = EquivClasses::new(false);
        let g = classes.intern(prez_source_ground(0));
        let l = classes.intern(prez_trans_lifted(7));

        assert!(!classes.merge(g, l).unwrap());
        assert_ne!(classes.find(g), classes.find(l));
    }

    /// `lhs cmp rhs + cst` is canonicalised into `sum(lhs) - sum(rhs) cmp cst`, with the constant columns' values folded into the constant.
    #[test]
    fn canonicalisation_negates_the_rhs_and_folds_known_values() {
        let mut classes = EquivClasses::new(true);
        // `x = y + 1`, i.e. `x - y = 1`
        let r = RowExpr::new(
            RowExprType::Eq,
            vec![(1, prez_trans_lifted(0)), (1, prez_trans_lifted(1))],
            1,
            1,
        );

        let canon = canonical(&r, &mut classes);
        assert_eq!(canon.cst, 1);
        assert_eq!(coef_of(&canon, &mut classes, prez_trans_lifted(0)), 1);
        assert_eq!(coef_of(&canon, &mut classes, prez_trans_lifted(1)), -1);

        // knowing `y = 1`, the very same row now reads `x = 2`
        let y = classes.intern(prez_trans_lifted(1));
        classes.set_value(y, 1).unwrap();

        let canon = canonical(&r, &mut classes);
        assert_eq!(canon.cst, 2);
        assert_eq!(coef_of(&canon, &mut classes, prez_trans_lifted(0)), 1);
        assert_eq!(coef_of(&canon, &mut classes, prez_trans_lifted(1)), 0); // gone into the constant
    }

    /// An equality row sitting at one of its bounds pins each of its columns to the end of its own range,
    /// while one of opposite coefficients merges them instead.
    #[test]
    fn an_equality_row_pins_or_merges_its_columns() {
        // `x + y = 2` leaves no choice but `x = y = 1`, and `x + y = 0` none but `x = y = 0`
        for (cst, pinned_to) in [(2, 1), (0, 0)] {
            let mut classes = EquivClasses::new(true);
            let r = row(
                RowExprType::Eq,
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
        // `x - y = 0` proves the two equal without fixing either
        let r = row(
            RowExprType::Eq,
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

    /// A row out of the reach of its columns is infeasible, be it from its coefficients alone or only once the known values have been folded in.
    #[test]
    fn a_row_out_of_the_reach_of_its_columns_is_infeasible() {
        let mut classes = EquivClasses::new(true);
        // `x >= 2` cannot hold for a column in `[0, 1]`
        let r = row(RowExprType::Geq, vec![(1, prez_trans_lifted(0))], 2);
        assert!(canonical(&r, &mut classes).learn(&mut classes).is_err());

        let mut classes = EquivClasses::new(true);
        let x = classes.intern(prez_trans_lifted(0));
        classes.set_value(x, 0).unwrap();
        // `x >= 1` with `x = 0` reads as the empty row `0 >= 1`
        let r = row(RowExprType::Geq, vec![(1, prez_trans_lifted(0))], 1);
        assert!(canonical(&r, &mut classes).learn(&mut classes).is_err());
    }
}
