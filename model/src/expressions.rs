use crate::bounds::Lit;
use crate::lang::Expr;
use std::collections::HashMap;
use std::convert::TryFrom;
use std::sync::Arc;

/// Datastructure that interns expressions and gives them a handle that can be used to retrieve the full expression.
///
/// Boolean expressions can optionally be associated with a literal that is constrained to have the same value.
#[derive(Default, Clone)]
pub struct Expressions {
    interned: HashMap<Arc<Expr>, Lit>,
    /// All binding events in chronological order. This is intended to easily process
    /// binding events and detect whether new events have been added.
    binding_events: Vec<(Lit, BindTarget)>,
}

#[derive(Clone, Debug)]
pub enum BindTarget {
    Expr(Arc<Expr>),
    Literal(Lit),
}

#[derive(Copy, Clone)]
pub struct BindingCursor(usize);

impl BindingCursor {
    pub fn first() -> Self {
        BindingCursor(0)
    }
}

impl Expressions {
    pub fn is_interned(&self, expr: &Expr) -> bool {
        self.interned.contains_key(expr)
    }

    pub fn handle_of(&self, expr: &Expr) -> Option<Lit> {
        if let Ok(lit) = Lit::try_from(expr) {
            Some(lit)
        } else {
            self.interned.get(expr).copied()
        }
    }

    pub fn set_handle(&mut self, expr: Arc<Expr>, literal: Lit) {
        assert!(self.handle_of(&expr).is_none());
        self.interned.insert(expr.clone(), literal);
        self.binding_events.push((literal, BindTarget::Expr(expr)));
    }

    pub fn bind(&mut self, expr: &Expr, literal: Lit) {
        if let Some(handle) = self.handle_of(expr) {
            self.bind_lit(literal, handle);
        } else {
            self.set_handle(Arc::new(expr.clone()), literal);
        }
    }
    pub fn bind_lit(&mut self, l1: Lit, l2: Lit) {
        self.binding_events.push((l1, BindTarget::Literal(l2)));
    }

    pub fn pop_next_event(&self, cursor: &mut BindingCursor) -> Option<&(Lit, BindTarget)> {
        let ret = self.binding_events.get(cursor.0);
        if ret.is_some() {
            cursor.0 += 1;
        }
        ret
    }
}
