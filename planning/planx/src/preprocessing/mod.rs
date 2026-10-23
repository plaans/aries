mod intify;
mod lift_predicates;

pub use intify::convert_reals_to_int;
pub use lift_predicates::lift_predicates_to_state_functions;

use crate::{Model, Res, SolutionPlan};

/// A model transformation that has been applied to a model.
///
/// This trait primarily aims at providing a way to revert the effect of the transformation
/// to obtain a solution of the original model.
pub trait Transformation {
    /// Human readable description of the transformation.
    fn describe(&self) -> String;

    /// Applies the inverse of the transformation to the plan, resulting in the plan that is valid in the model before this
    /// transformation was applied.
    fn revert(&self, plan: SolutionPlan) -> SolutionPlan;
}

#[derive(Debug, Copy, Clone)]
pub enum ModelTransformation {
    /// Attempts to lift predicates into multi-valued state variables. See [`lift_predicates_to_state_functions`].
    LiftPredicates,
    /// Attempts to remove all non-integer numbers for the model. See [`convert_reals_to_int`].
    Intify,
}

/// Applies a built-in transformation to the model.
pub fn apply_transformation(mut model: Model, transformation: ModelTransformation) -> Res<Model> {
    match transformation {
        ModelTransformation::LiftPredicates => {
            lift_predicates_to_state_functions(&mut model)?;
            Ok(model)
        }
        ModelTransformation::Intify => convert_reals_to_int(&model),
    }
}

/// Applies a sequence of built-in transformations to the model.
pub fn apply_transformations(mut model: Model, transformations: &[ModelTransformation]) -> Res<Model> {
    for tr in transformations {
        model = apply_transformation(model, *tr)?;
    }
    Ok(model)
}
