use crate::bounds::Lit;
use crate::expressions::ExprHandle;
use aries_collections::ref_store::RefMap;

#[derive(Copy, Clone, Debug)]
pub enum BindTarget {
    Expr(ExprHandle),
    Literal(Lit),
}

#[derive(Copy, Clone)]
pub struct BindingCursor(usize);

impl BindingCursor {
    pub fn first() -> Self {
        BindingCursor(0)
    }
}

/// A structure to keep track of bindings between a literal and an expression or between to literals.
#[derive(Clone, Default)]
pub struct Bindings {
    /// If the expression was reified, associates the handle with the equivalent literal
    bindings: RefMap<ExprHandle, Lit>,
    /// All binding events in chronological order. This is intended to easily process
    /// binding events and detect whether new events have been added.
    binding_events: Vec<(Lit, BindTarget)>,
}

impl Bindings {
    pub fn as_lit(&self, expr_id: ExprHandle) -> Option<Lit> {
        self.bindings.get(expr_id).copied()
    }

    pub fn bind(&mut self, expr_id: ExprHandle, literal: Lit) {
        assert!(!self.bindings.contains(expr_id));
        self.bindings.insert(expr_id, literal);
        self.binding_events.push((literal, BindTarget::Expr(expr_id)));
    }

    pub fn bind_literals(&mut self, l1: Lit, l2: Lit) {
        self.binding_events.push((l1, BindTarget::Literal(l2)))
    }

    pub fn pop_next_event(&self, cursor: &mut BindingCursor) -> Option<(Lit, BindTarget)> {
        let ret = self.binding_events.get(cursor.0).copied();
        if ret.is_some() {
            cursor.0 += 1;
        }
        ret
    }
}
