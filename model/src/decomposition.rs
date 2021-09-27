use crate::bounds::Lit;
use crate::extensions::AssignmentExt;
use crate::label::Label;
use crate::lang::expr::{and, leq, opt_leq};
use crate::lang::normal_form::{NFEq, NFOptEq};
use crate::lang::reification::{downcast, BindTarget, BindingCursor, Expr};
use crate::lang::IVar;
use crate::Model;
use std::sync::Arc;

/// Module to constructs the constraints from the model and store them in a queue.
#[derive(Clone)]
pub struct Constraints<Lbl> {
    /// Object in charge of transforming high-level constraints into simpler ones that the solver can handle
    decomposer: Arc<dyn Decompose<Lbl>>,
    /// A queue of constraints that has been decomposed from the model's bindings.
    constraints: Vec<(Lit, BindTarget)>,
    /// Cursor into the model's bindings
    next_to_decompose: BindingCursor,
    /// Cursor into the constraints queue, that can be pulled by a solver to import constraints
    next_constraint: usize,
}

impl<Lbl> Constraints<Lbl> {
    pub fn new<T: Decompose<Lbl> + 'static>(decomposer: T) -> Self {
        Constraints {
            decomposer: Arc::new(decomposer),
            constraints: vec![],
            next_to_decompose: BindingCursor::first(),
            next_constraint: Default::default(),
        }
    }

    /// Pulls all bindings in the model, apply decomposition and load maximally decomposed constraints into our queue.
    ///
    /// Not that in the process, the subconstraints resulting from the decomposition are added to the model.
    pub fn decompose_all(&mut self, model: &mut Model<Lbl>) {
        while let Some((lit, target)) = model
            .shape
            .expressions
            .pop_next_event(&mut self.next_to_decompose)
            .cloned()
        {
            match target {
                BindTarget::Expr(e) => {
                    match self.decomposer.decompose(lit, e.as_ref(), model) {
                        DecompositionResult::Decomposed => {
                            // constraint was decomposed and subconstraints were added to the model's expressions.
                            // do nothing as we will encounter them later in the loop
                        }
                        DecompositionResult::Inapplicable => {
                            // could not be further decomposed, add it as is to the constraints
                            self.constraints.push((lit, BindTarget::Expr(e)));
                        }
                    }
                }
                BindTarget::Literal(l2) => {
                    // nothing to decompose, transfer it immediately
                    self.constraints.push((lit, BindTarget::Literal(l2)))
                }
            }
        }
    }

    /// Removes and returns the next constraint in the queue.
    pub fn pop_next_constraint(&mut self) -> Option<&(Lit, BindTarget)> {
        let ret = self.constraints.get(self.next_constraint);
        if ret.is_some() {
            self.next_constraint += 1;
        }
        ret
    }
}

impl<Lbl: Label> Default for Constraints<Lbl> {
    fn default() -> Self {
        Self::new(Eq2Leq)
    }
}

pub enum DecompositionResult {
    Decomposed,
    Inapplicable,
}

pub trait Decompose<Lbl>: Send + Sync {
    fn decompose(&self, binding: Lit, expression: &Expr, model: &mut Model<Lbl>) -> DecompositionResult;
}

pub struct Eq2Leq;

impl<Lbl: Label> Decompose<Lbl> for Eq2Leq {
    fn decompose(&self, literal: Lit, expr: &Expr, model: &mut Model<Lbl>) -> DecompositionResult {
        if let Some(&NFEq { lhs, rhs, rhs_add }) = downcast(expr) {
            // decompose `l <=> (a = b)` into `l <=> (a <= b) && (b <= a)`
            let lhs = IVar::new(lhs);
            let rhs = IVar::new(rhs) + rhs_add;
            let x = model.reify(leq(lhs, rhs));
            let y = model.reify(leq(rhs, lhs));
            model.bind(and([x, y]), literal);
            DecompositionResult::Decomposed
        } else if let Some(&NFOptEq { lhs, rhs, rhs_add }) = downcast(expr) {
            if model.entails(literal) {
                let a = IVar::new(lhs);
                let b = IVar::new(rhs) + rhs_add;
                model.enforce(opt_leq(a, b));
                model.enforce(opt_leq(b, a));
                DecompositionResult::Decomposed
            } else {
                DecompositionResult::Inapplicable
            }
        } else {
            DecompositionResult::Inapplicable
        }
    }
}
