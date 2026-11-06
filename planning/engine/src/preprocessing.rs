use planx::{Model, Res, SolutionPlan, preprocessing::ModelTransformation};

/// Model transformations that are applied by default to any model
/// that requires encoding into the solver (validation, optimization, solving, ...).
pub const DEFAULT_MODEL_TRANSFORMATIONS: &[ModelTransformation] =
    // &[ModelTransformation::LiftPredicates, ModelTransformation::Intify];
    &[ModelTransformation::Intify];

/// Applies a sequence of transformations to the model.
pub fn preprocess_model(model: Model, transformations: &[ModelTransformation]) -> Res<Model> {
    planx::preprocessing::apply_transformations(model, transformations)
}

/// Applies a sequence of transformation to a model and a plan.
pub fn preprocess(
    mut model: Model,
    mut plan: SolutionPlan,
    transformations: &[ModelTransformation],
) -> Res<(Model, SolutionPlan)> {
    model.required_temporal_precision.union(plan.required_time_precision());
    let model = preprocess_model(model, transformations)?;
    for tr in &model.transformations {
        plan = tr.apply(plan)?;
    }
    Ok((model, plan))
}
