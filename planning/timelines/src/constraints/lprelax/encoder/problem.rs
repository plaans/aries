mod simplify;

use std::collections::HashMap;

use aries_solver::core::IntCst;
use aries_solver::core::state::Domains;

use crate::IntTerm;
use crate::analysis::transitions::TransitionId;
use crate::encoder::SchedEncoder;
use crate::{analysis::Source, constraints::lprelax::LpRelaxEncoder};

use super::ground::{SourceGroundingId, TransitionGroundingId};

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

    /// Numbers the columns of the rows as they are, without simplifying anything:
    /// every tag is its own column.
    ///
    /// Afterwards, [`Self::cols`] and `col_index` are available just as after [`Self::simplify`].
    pub fn seal(&mut self) {
        self.number_cols(std::iter::empty());
    }

    /// Numbers the columns appearing in the rows, in order of first appearance.
    /// Each `(alias, tag)` pair then makes `alias` resolve to the column of `tag`
    /// (the simplification uses this for tags merged into their class' representative).
    fn number_cols(&mut self, aliases: impl IntoIterator<Item = (ColTag, ColTag)>) {
        let mut cols = vec![];
        let mut col_index = HashMap::new();

        for row in &self.rows {
            for &(_, col_tag) in &row.terms {
                col_index.entry(col_tag).or_insert_with(|| {
                    cols.push(col_tag);
                    cols.len() - 1
                });
            }
        }

        for (alias, col_tag) in aliases {
            if let Some(&col) = col_index.get(&col_tag) {
                col_index.insert(alias, col);
            }
        }

        self.cols = Some(cols);
        self.col_index = Some(col_index);
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
    ///
    /// See [`Self::seal`] to get a usable problem without simplifying anything.
    pub fn simplify(
        &mut self,
        encoder: &LpRelaxEncoder,
        ctx: &SchedEncoder,
        doms: &Domains,
        merge_equal_columns: bool,
    ) {
        let prev_rows_len = self.rows().len();
        let time = std::time::Instant::now();

        if simplify::try_simplify(self, encoder, ctx, doms, merge_equal_columns).is_err() {
            self.make_infeasible();
        }

        tracing::info!(
            "|-[LPRELAX]--- LPrelax problem simplification: {} rows removed in {}s (remaining: {} rows and {} columns)",
            prev_rows_len - self.rows().len(),
            time.elapsed().as_secs_f64(),
            self.rows().len(),
            self.cols().as_ref().unwrap().len(),
        );
    }

    /// Replaces the problem by two rows over a single column, `x <= 0` and `x >= 1`:
    /// infeasible whatever the column's bounds, while each row on its own has consistent bounds
    /// (a row with inconsistent bounds makes HiGHS crash).
    fn make_infeasible(&mut self) {
        let col_tag = ColTag::PresenceSource(None, None);

        self.rows = vec![
            RowExpr::new(RowExprType::Leq, vec![(1, col_tag)], 1, 0),
            RowExpr::new(RowExprType::Geq, vec![(1, col_tag)], 1, 1),
        ];
        self.seal();
    }
}
