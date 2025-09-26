use idmap::DirectIdMap;

use crate::{Expr, ExprId, ExprNode, Fluents, Message, Objects, Types, errors::Span};

pub struct Environment {
    pub types: Types,
    pub objects: Objects,
    pub fluents: Fluents,
    exprs: DirectIdMap<ExprId, ExprNode>,
    next_expr_id: u32,
}

#[derive(Copy, Clone)]
pub struct Env<'a, T> {
    pub elem: T,
    pub env: &'a Environment,
}

impl Environment {
    pub fn new(types: Types) -> Self {
        Self {
            types,
            objects: Default::default(),
            fluents: Default::default(),
            exprs: Default::default(),
            next_expr_id: 0u32,
        }
    }
    pub(super) fn get(&self, id: ExprId) -> &ExprNode {
        self.exprs.get(id).unwrap()
    }

    pub fn node<T>(&self, id: T) -> Env<'_, T> {
        self / id
    }

    pub fn intern(&mut self, expr: Expr, span: impl Into<Option<Span>>) -> Result<ExprId, Message> {
        let tpe = expr.tpe(self)?;
        let id = ExprId(self.next_expr_id);
        self.next_expr_id += 1;
        let res = self.exprs.insert(id, ExprNode::new(expr, tpe, span.into()));
        debug_assert!(res.is_none());
        Ok(id)
    }
}

impl<'a, T> std::ops::Div<T> for &'a Environment {
    type Output = Env<'a, T>;

    fn div(self, rhs: T) -> Self::Output {
        Env { elem: rhs, env: self }
    }
}

impl<'a, T> std::ops::Div<T> for &'a mut Environment {
    type Output = Env<'a, T>;

    fn div(self, rhs: T) -> Self::Output {
        Env { elem: rhs, env: self }
    }
}
