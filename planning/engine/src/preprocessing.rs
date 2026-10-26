use planx::{Model, Res, preprocessing::ModelTransformation};

/// Model transformations that are applied by default to any model
/// that requires encoding into the solver (validation, optimization, solving, ...).
pub const DEFAULT_MODEL_TRANSFORMATIONS: &[ModelTransformation] =
    &[ModelTransformation::LiftPredicates, ModelTransformation::Intify];

/// Applies a sequence of transformations to the model.
pub fn preprocess(model: Model, transformations: &[ModelTransformation]) -> Res<Model> {
    planx::preprocessing::apply_transformations(model, transformations)
}
