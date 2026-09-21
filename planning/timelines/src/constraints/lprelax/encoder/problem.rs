use std::collections::HashMap;

use aries_solver::core::IntCst;

use crate::analysis::Source;
use crate::analysis::transitions::TransitionId;
use crate::{Domains, IntTerm, SchedEncoder};

use super::LpRelaxEncoder;
use super::groundings::{SourceGroundingId, TransitionGroundingId};

/// Represents a variable / column
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub enum ColTag {
    PresenceSource(Source, Option<SourceGroundingId>),
    PresenceTransition(TransitionId, Option<TransitionGroundingId>),
    Support(
        TransitionId,
        TransitionId,
        Option<(TransitionGroundingId, TransitionGroundingId)>,
    ),
    TermGround(IntTerm, IntCst),
}
impl ColTag {
    /// Whether this column tag is lifted (i.e. isn't specific to a grounding, i.e. corresponds to a variable in the main model).
    pub fn is_lifted(&self) -> bool {
        match self {
            ColTag::PresenceSource(_, grounding) => grounding.is_none(),
            ColTag::PresenceTransition(_, grounding) => grounding.is_none(),
            ColTag::Support(_, _, groundings) => groundings.is_none(),
            ColTag::TermGround(_, _) => true,
        }
    }
}

/// Represents an expression of the form `lhs cmp rhs + cst`.
/// where `cmp` can be `=`, `=`, or `>=`, and coefficient of terms in `lhs` and `rhs` is 1.
#[derive(Debug, Clone)]
pub struct RowExpr {
    pub tpe: RowExprType,
    /// Index separating the lhs and rhs terms. As such, `terms[separator]` must be the first term of the rhs.
    separator: usize,
    terms: Vec<(IntCst, ColTag)>,
    /// Constant part of the expression, in the rhs
    cst: IntCst,
}
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RowExprType {
    Eq,
    Leq,
    Geq,
}
impl RowExpr {
    pub fn lhs(&self) -> &[(IntCst, ColTag)] {
        self.terms.get(..self.separator).unwrap_or(&[])
    }
    pub fn rhs(&self) -> &[(IntCst, ColTag)] {
        self.terms.get(self.separator..).unwrap_or(&[])
    }
    pub fn cst(&self) -> IntCst {
        self.cst
    }
    #[allow(dead_code)]
    pub fn new(tpe: RowExprType, terms: Vec<(IntCst, ColTag)>, separator: usize, cst: IntCst) -> Self {
        Self {
            tpe,
            separator,
            terms,
            cst,
        }
    }
    pub fn new_eq_single_lhs(terms: Vec<(IntCst, ColTag)>) -> Self {
        Self {
            tpe: RowExprType::Eq,
            separator: 1,
            terms,
            cst: 0,
        }
    }
    pub fn new_leq_single_lhs(terms: Vec<(IntCst, ColTag)>) -> Self {
        Self {
            tpe: RowExprType::Leq,
            separator: 1,
            terms,
            cst: 0,
        }
    }
    pub fn new_geq_single_lhs(terms: Vec<(IntCst, ColTag)>) -> Self {
        Self {
            tpe: RowExprType::Geq,
            separator: 1,
            terms,
            cst: 0,
        }
    }
    pub fn new_leq_1(terms: Vec<(IntCst, ColTag)>) -> Self {
        Self {
            tpe: RowExprType::Leq,
            separator: terms.len(),
            terms,
            cst: 1,
        }
    }
}

/// Represents a problem
#[derive(Clone, Default)]
pub struct LpRelaxProblem {
    /// One entry per LP column (only once simplified).
    cols: Option<Vec<ColTag>>,
    /// Maps every column tag appearing in the rows to the index, in `cols`, of the column it resolves to.
    /// With column merging enabled, several tags resolve to the same column;
    /// keeping all of them here lets each of their bindings to the main model constrain that column.
    col_index: Option<HashMap<ColTag, usize>>,
    rows: Vec<RowExpr>,
}
impl LpRelaxProblem {
    pub fn push_row(&mut self, row: RowExpr) {
        self.cols = None;
        self.col_index = None;
        self.rows.push(row);
    }
    pub fn rows(&self) -> &[RowExpr] {
        &self.rows
    }
    pub fn cols(&self) -> Option<&[ColTag]> {
        self.cols.as_deref()
    }

    /// Simplifies the problem, given what the domains already fix:
    ///
    /// - a column whose value is known is replaced by that value in every row;
    /// - a row sitting at one of its bounds (i.e. equality row) pins all of its columns (e.g. `x + y = 0`, or a single-term `x = 1`) -- which in turn feeds the point above;
    /// - rows that became empty are dropped, and a row that became contradictory turns the whole problem into a single trivially infeasible one;
    /// - when `merge_equal_columns` is set, a row of the form `c*x - c*y = 0` merges the two columns into a single one,
    ///   (represented by a "lifted" column tag when possible).
    ///
    /// Afterwards, [`Self::cols`] holds one entry per remaining LP column.
    pub fn simplify(
        &mut self,
        encoder: &LpRelaxEncoder,
        ctx: &SchedEncoder,
        doms: &Domains,
        merge_equal_columns: bool,
    ) {
        let mut classes = EquivClasses::new(merge_equal_columns);

        let prev_rows_len = self.rows().len();
        let time = std::time::Instant::now();

        if self.try_simplify(&mut classes, encoder, ctx, doms).is_err() {
            self.make_infeasible();
        }

        println!(
            "|-[LPRELAX]--- LPrelax problem simplification: {} rows removed in {}s (remaining: {} rows and {} columns)",
            prev_rows_len - self.rows().len(),
            time.elapsed().as_secs_f64(),
            self.rows().len(),
            self.cols().as_ref().unwrap().len(),
        );
    }

    fn try_simplify(
        &mut self,
        classes: &mut EquivClasses,
        encoder: &LpRelaxEncoder,
        ctx: &SchedEncoder,
        doms: &Domains,
    ) -> Result<(), Infeasible> {
        self.seed_known_values(classes, encoder, ctx, doms)?;

        // Learn values (and equalities) from the rows until nothing new comes out.
        // Each round re-reads the original rows, whose canonical form only gets simpler as knowledge grows.
        loop {
            let mut learned = false;
            for row in &self.rows {
                learned |= CanonicalRow::of(row, classes).learn(classes)?;
            }
            if !learned {
                break;
            }
        }

        // Rewrite the rows over the columns that are left, and number those columns.
        let mut rows = Vec::with_capacity(self.rows.len());
        let mut cols = vec![];
        let mut col_of_class = HashMap::new();

        for row in &self.rows {
            let canon = CanonicalRow::of(row, classes);
            if canon.coefs.is_empty() {
                // Empty: `learn` has already checked that the constant satisfies the comparison.
                continue;
            }

            let mut terms = Vec::with_capacity(canon.coefs.len());
            for &(coef, class) in &canon.coefs {
                let col = match col_of_class.get(&class) {
                    Some(&col) => col,
                    None => {
                        let col = cols.len();
                        cols.push(classes.representative(class));
                        col_of_class.insert(class, col);
                        col
                    }
                };
                terms.push((coef, cols[col]));
            }

            let separator = terms.len();
            rows.push(RowExpr::new(canon.tpe, terms, separator, canon.cst));
        }

        // Every "surviving" column tag resolves to its equivalence class' representative column.
        let mut col_index = HashMap::new();
        for (tag, class) in classes.interned() {
            if let Some(&col) = col_of_class.get(&classes.find(class)) {
                col_index.insert(tag, col);
            }
        }

        self.rows = rows;
        self.cols = Some(cols);
        self.col_index = Some(col_index);

        Ok(())
    }

    /// Records the values that the domains already fix for the lifted presence and support columns,
    /// and for the term grounding columns appearing in the rows.
    fn seed_known_values(
        &self,
        classes: &mut EquivClasses,
        encoder: &LpRelaxEncoder,
        ctx: &SchedEncoder,
        doms: &Domains,
    ) -> Result<(), Infeasible> {
        let presence_cols = encoder
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

        for (col_tag, lit) in presence_cols {
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

            let class = classes.intern(col_tag);
            classes.set_value(class, value)?;
        }

        for row in &self.rows {
            for &(_, col_tag) in &row.terms {
                let ColTag::TermGround(term, value) = col_tag else {
                    continue;
                };

                // Out of the term's bounds: the column is 0 whether or not the term is present.
                // (It is 1 only if the term is known present and fixed to that value).
                let known = if doms.entails(!doms.presence(term)) || value < doms.lb(term) || doms.ub(term) < value {
                    0
                } else if doms.entails(doms.presence(term)) {
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

    /// Replaces the problem by a single `x >= 2` row over a column of `[0, 1]`: infeasible, and in
    /// a shape HiGHS handles (a row with inconsistent bounds makes it crash).
    fn make_infeasible(&mut self) {
        let col_tag = ColTag::PresenceSource(None, None);

        self.rows = vec![RowExpr::new(RowExprType::Geq, vec![(1, col_tag)], 1, 2)];
        self.cols = Some(vec![col_tag]);
        self.col_index = Some(HashMap::from([(col_tag, 0)]));
    }
}

/// The problem was found trivially infeasible during simplification.
struct Infeasible;

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
    values: HashMap<u32, IntCst>,
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
            values: HashMap::new(),
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
        self.values.get(&root).copied()
    }

    /// The tag that the class of `i` is represented by (a lifted one whenever the class holds one).
    fn representative(&mut self, i: u32) -> ColTag {
        let root = self.find(i);
        self.tags[self.representative[root as usize] as usize]
    }

    /// Records that the class of `i` takes value `v`. Returns whether that was new.
    fn set_value(&mut self, i: u32, v: IntCst) -> Result<bool, Infeasible> {
        if !(0..=1).contains(&v) {
            return Err(Infeasible); // columns live in `[0, 1]`
        }
        let root = self.find(i);
        match self.values.insert(root, v) {
            Some(old) if old != v => Err(Infeasible),
            Some(_) => Ok(false),
            None => Ok(true),
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
        if let (Some(va), Some(vb)) = (self.values.get(&kept), self.values.get(&merged))
            && va != vb
        {
            return Err(Infeasible);
        }

        if self.size[kept as usize] < self.size[merged as usize] {
            std::mem::swap(&mut kept, &mut merged);
        }
        self.parent[merged as usize] = kept;
        self.size[kept as usize] += self.size[merged as usize];

        if let Some(v) = self.values.remove(&merged) {
            self.values.insert(kept, v);
        }

        // Of the two representatives, prefer a lifted tag; break ties deterministically.
        let (r_kept, r_merged) = (self.representative[kept as usize], self.representative[merged as usize]);
        let key = |i: u32| {
            let tag = self.tags[i as usize];
            (!tag.is_lifted(), tag)
        };
        self.representative[kept as usize] = if key(r_kept) <= key(r_merged) { r_kept } else { r_merged };

        Ok(true)
    }
}

/// A row rewritten with the representative columns / tags of the union-find
struct CanonicalRow {
    tpe: RowExprType,
    coefs: Vec<(IntCst, u32)>,
    cst: IntCst,
}
impl CanonicalRow {
    fn of(row: &RowExpr, classes: &mut EquivClasses) -> Self {
        let mut coefs: Vec<(IntCst, u32)> = Vec::with_capacity(row.terms.len());
        let mut cst = row.cst();

        for (i, &(coef, col_tag)) in row.terms.iter().enumerate() {
            // `lhs cmp rhs + cst` reads as `sum(lhs) - sum(rhs) cmp cst`.
            let coef = if i < row.separator { coef } else { -coef };
            let class = classes.intern(col_tag);

            match classes.value(class) {
                Some(value) => cst -= coef * value,
                None => {
                    let root = classes.find(class);
                    match coefs.iter_mut().find(|(_, c)| *c == root) {
                        Some((c, _)) => *c += coef,
                        None => coefs.push((coef, root)),
                    }
                }
            }
        }
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
            return Err(Infeasible);
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
