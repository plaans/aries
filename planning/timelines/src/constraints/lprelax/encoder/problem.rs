mod simplify;

use std::hash::Hash;

use hashbrown::HashMap;

use aries_solver::core::IntCst;

use crate::analysis::Source;
use crate::lprelax::transitions::TransitionId;
use crate::{Domains, IntTerm, SchedEncoder};

use super::LpRelaxEncoder;
use super::ground::{SourceGroundingId, StateVarGroundingId, SupportGroundingId, TransitionGroundingId};

/// Represents a variable / column
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub enum ColTag {
    /// Presence of a source: lifted (`None`), or of one of its groundings (`Some`).
    PresenceSource(Source, Option<SourceGroundingId>),
    /// Presence of a transition: lifted (`None`), or of one of its groundings (`Some`).
    PresenceTransition(TransitionId, Option<TransitionGroundingId>),
    /// Support from a transition to another:
    /// lifted (`None`), or ground (`Some`, for a given ground state variable and a given handed-over value).
    Support(TransitionId, TransitionId, Option<SupportGroundingId>),
    /// Support of an effect by the initial state of a ground state variable grounding (when `Some`), or of any of them (lifted, when `None`).
    /// On each ground state variable, the initial state supports at most one effect (whatever its source).
    SupportFromInitialState(Option<StateVarGroundingId>, TransitionId),
    /// Whether a term is assigned a value (ground).
    TermGround(IntTerm, IntCst),
}
impl ColTag {
    /// Whether this column tag is lifted -- i.e. isn't specific to a grounding -- or is a term grounding.
    pub fn is_lifted_or_term_grounding(&self) -> bool {
        match self {
            ColTag::PresenceSource(_, grounding) => grounding.is_none(),
            ColTag::PresenceTransition(_, grounding) => grounding.is_none(),
            ColTag::Support(_, _, grounding) => grounding.is_none(),
            ColTag::SupportFromInitialState(state_var_grounding, _) => state_var_grounding.is_none(),
            ColTag::TermGround(_, _) => true,
        }
    }

    /// The upper bound of the column (`None` if it has none). Every column has 0 as lower bound.
    pub fn upper_bound(&self) -> Option<IntCst> {
        match self {
            ColTag::PresenceSource(..)
            | ColTag::PresenceTransition(..)
            | ColTag::Support(..)
            | ColTag::SupportFromInitialState(..)
            | ColTag::TermGround(..) => Some(1),
        }
    }
}

/// Represents an expression of the form `lhs cmp rhs + cst`,
/// where `cmp` can be `=`, `<=`, or `>=`, and `lhs` and `rhs` are weighted sums of columns (designated by `C`).
#[derive(Debug, Clone)]
pub struct RowExpr<C = ColTag> {
    pub tpe: RowExprType,
    /// Index separating the lhs and rhs terms. As such, `terms[separator]` must be the first term of the rhs.
    separator: usize,
    terms: Vec<(IntCst, C)>,
    /// Constant part of the expression, in the rhs
    cst: IntCst,
}
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RowExprType {
    Eq,
    Leq,
    Geq,
}
impl<C> RowExpr<C> {
    pub fn lhs(&self) -> &[(IntCst, C)] {
        self.terms.get(..self.separator).unwrap_or(&[])
    }
    pub fn rhs(&self) -> &[(IntCst, C)] {
        self.terms.get(self.separator..).unwrap_or(&[])
    }
    pub fn cst(&self) -> IntCst {
        self.cst
    }
    pub fn new(tpe: RowExprType, terms: Vec<(IntCst, C)>, separator: usize, cst: IntCst) -> Self {
        Self {
            tpe,
            separator,
            terms,
            cst,
        }
    }
    /// `terms[0] = terms[1] + ... + terms[n]`
    pub fn new_eq_single_lhs(terms: Vec<(IntCst, C)>) -> Self {
        Self {
            tpe: RowExprType::Eq,
            separator: 1,
            terms,
            cst: 0,
        }
    }
    /// `terms[0] <= terms[1] + ... + terms[n]`
    pub fn new_leq_single_lhs(terms: Vec<(IntCst, C)>) -> Self {
        Self {
            tpe: RowExprType::Leq,
            separator: 1,
            terms,
            cst: 0,
        }
    }
    /// `terms[0] >= terms[1] + ... + terms[n]`
    pub fn new_geq_single_lhs(terms: Vec<(IntCst, C)>) -> Self {
        Self {
            tpe: RowExprType::Geq,
            separator: 1,
            terms,
            cst: 0,
        }
    }
    /// `terms[0] + ... + terms[n] <= 1`
    pub fn new_leq_1(terms: Vec<(IntCst, C)>) -> Self {
        Self {
            tpe: RowExprType::Leq,
            separator: terms.len(),
            terms,
            cst: 1,
        }
    }
}

/// Represents a problem: its rows, over columns designated by `C`.
#[derive(Clone)]
pub struct LpRelaxProblem<C = ColTag> {
    /// One entry per LP column (`Some` only once simplified or sealed).
    cols: Option<Vec<C>>,
    /// Maps every column tag appearing in the rows to the index, in `cols`, of the column it resolves to.
    col_index: Option<HashMap<C, usize>>,
    /// The values that the simplification found a column tag to take (given what the domains fixed at the time).
    /// Such a tag isn't included in the problem's actual columns in `cols` though (as it's been determined to be a constant).
    col_known_value: HashMap<C, IntCst>,
    rows: Vec<RowExpr<C>>,
}
// (Not derived, as that would require `C: Default`.)
impl<C> Default for LpRelaxProblem<C> {
    fn default() -> Self {
        Self {
            cols: None,
            col_index: None,
            col_known_value: HashMap::new(),
            rows: vec![],
        }
    }
}
impl<C> LpRelaxProblem<C> {
    pub fn push_row(&mut self, row: RowExpr<C>) {
        self.cols = None;
        self.col_index = None;
        self.col_known_value.clear();
        self.rows.push(row);
    }
    pub fn rows(&self) -> &[RowExpr<C>] {
        &self.rows
    }
    pub fn cols(&self) -> Option<&[C]> {
        self.cols.as_deref()
    }
    /// Maps every column tag that resolves to a column to that column's index in [`Self::cols`].
    pub fn col_index(&self) -> Option<&HashMap<C, usize>> {
        self.col_index.as_ref()
    }
}
impl<C: Copy + Eq + Hash> LpRelaxProblem<C> {
    /// The column tags whose necessary value was found by the simplification
    /// (such tags aren't held as columns in the simplified problem anymore, though).
    pub fn col_known_values(&self) -> impl Iterator<Item = (C, IntCst)> {
        self.col_known_value.iter().map(|(&tag, &value)| (tag, value))
    }

    /// Numbers the columns of the rows as they are, without simplifying anything: every tag is its own column.
    ///
    /// Afterwards, [`Self::cols`] and [`Self::col_index`] are available just as after [`Self::simplify`].
    pub fn seal(&mut self) {
        self.number_cols(std::iter::empty());
    }

    /// Numbers the columns appearing in the rows, by first appearance.
    /// Each `(alias, tag)` pair then makes `alias` resolve to the column of `tag`
    /// (the simplification uses this for tags merged into their class' representative).
    fn number_cols(&mut self, aliases: impl IntoIterator<Item = (C, C)>) {
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
}
impl LpRelaxProblem {
    /// Simplifies the problem, given what the domains already fix:
    ///
    /// - a column whose value is known is replaced by that value in every row;
    /// - a row sitting at one of its bounds pins all of its columns (e.g. `x + y = 0`, or a single-term `x = 1`) -- which in turn feeds the point above;
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

        lprelax_log!(
            "LpRelaxProblem simplification: {} rows removed in {} (remaining: {} rows and {} columns)",
            prev_rows_len.saturating_sub(self.rows().len()),
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
        self.col_known_value.clear();
        self.seal();
    }
}
