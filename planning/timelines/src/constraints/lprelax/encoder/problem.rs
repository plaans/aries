use aries_solver::core::IntCst;

/// Represents an expression of the form `lhs cmp rhs + cst`,
/// where `cmp` can be `=`, `<=`, or `>=`, and `lhs` and `rhs` are weighted sums of columns (designated by `C`).
#[derive(Debug, Clone)]
pub struct RowExpr<C> {
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
pub struct LpRelaxProblem<C> {
    rows: Vec<RowExpr<C>>,
}
// (Not derived, as that would require `C: Default`.)
impl<C> Default for LpRelaxProblem<C> {
    fn default() -> Self {
        Self { rows: vec![] }
    }
}
impl<C> LpRelaxProblem<C> {
    pub fn push_row(&mut self, row: RowExpr<C>) {
        self.rows.push(row);
    }
    pub fn rows(&self) -> &[RowExpr<C>] {
        &self.rows
    }
}
