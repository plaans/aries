use crate::lang::{BAtom, BVar, Expr};
use std::collections::HashMap;

#[derive(Default, Clone)]
pub struct Expressions {
    interned: HashMap<Expr, BVar>,
    expressions: HashMap<BVar, Expr>,
}
#[derive(Ord, PartialOrd, Eq, PartialEq)]
pub enum NExpr<'a> {
    Pos(&'a Expr),
    Neg(&'a Expr),
}

/// Identifier of an expression which can be retrieved with [Expressions::get]
#[derive(Copy, Clone)]
pub struct ExprHandle(BVar);

impl Expressions {
    pub fn contains_expr(&self, expr: &Expr) -> bool {
        self.interned.contains_key(expr)
    }

    pub fn variable_of(&self, expr: &Expr) -> Option<BVar> {
        self.interned.get(expr).copied()
    }

    pub fn get(&self, expr_id: ExprHandle) -> &Expr {
        self.expressions.get(&expr_id.0).unwrap()
    }

    pub fn expr_of_variable(&self, atom: BVar) -> Option<ExprHandle> {
        if self.expressions.contains_key(&atom) {
            Some(ExprHandle(atom))
        } else {
            None
        }
    }

    pub fn expr_of(&self, atom: impl Into<BAtom>) -> Option<NExpr> {
        let atom = atom.into();
        atom.var
            .and_then(|v| self.expressions.get(&v))
            .map(|e| if atom.negated { NExpr::Neg(e) } else { NExpr::Pos(e) })
    }

    pub fn bind(&mut self, var: BVar, expr: Expr) {
        self.interned.insert(expr.clone(), var);
        self.expressions.insert(var, expr);
    }
}
