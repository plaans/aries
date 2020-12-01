use crate::model::lang::{Atom, Expr};
use std::collections::HashMap;

#[derive(Default)]
pub struct Expressions {
    interned: HashMap<Expr, Atom>,
    expressions: HashMap<Atom, Expr>,
}
impl Expressions {
    pub fn contains_expr(&self, expr: &Expr) -> bool {
        self.interned.contains_key(expr)
    }

    pub fn atom_of(&self, expr: &Expr) -> Option<Atom> {
        self.interned.get(expr).copied()
    }

    pub fn expr_of(&self, atom: impl Into<Atom>) -> Option<&Expr> {
        self.expressions.get(&atom.into())
    }

    pub fn bind(&mut self, atom: Atom, expr: Expr) {
        self.interned.insert(expr.clone(), atom);
        self.expressions.insert(atom, expr);
    }
}
