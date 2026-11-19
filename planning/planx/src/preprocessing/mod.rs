pub mod analysis;
mod annotate_transitions;
mod erase_unusable_effects;
mod intify;
mod lift_predicates;
mod merge_conditions;
mod merge_effect_condition;
mod merge_erase_assign;
mod split_predicates;

pub use annotate_transitions::annotate_transitions;
pub use erase_unusable_effects::erase_unusable_effects;
pub use intify::convert_reals_to_int;
pub use lift_predicates::lift_predicates_to_state_functions;
pub use merge_conditions::merge_conditions;
pub use merge_effect_condition::merge_effect_conditions;
pub use merge_erase_assign::merge_erase_assign_effects;
pub use split_predicates::split_predicates;

use crate::{Model, Res, SolutionPlan};

/// A model transformation that has been applied to a model.
///
/// This trait primarily aims at providing a way to revert the effect of the transformation
/// to obtain a solution of the original model.
pub trait Transformation {
    /// Human readable description of the transformation.
    fn describe(&self) -> String;

    /// Applies the transformation to the given plan.
    fn apply(&self, plan: SolutionPlan) -> Res<SolutionPlan>;

    /// Applies the inverse of the transformation to the plan, resulting in the plan that is valid in the model before this
    /// transformation was applied.
    fn revert(&self, plan: SolutionPlan) -> Res<SolutionPlan>;
}

#[derive(Debug, Copy, Clone)]
pub enum ModelTransformation {
    /// Splits predicates according to the types of their arguments. See [`split_predicates`].
    SplitPredicates,
    /// Attempts to lift predicates into multi-valued state variables. See [`lift_predicates_to_state_functions`].
    LiftPredicates,
    /// Attempts to remove all non-integer numbers for the model. See [`convert_reals_to_int`].
    Intify,
    /// Replaces assignment effects whose value can never be read by erase effects. See [`erase_unusable_effects`].
    EraseUnusableEffects,
    /// Merges conditions on the same expression over overlapping or contiguous intervals. See [`merge_conditions`].
    MergeConditions,
    /// Merges an assignment and the following condition requiring its value into an erasure.
    /// See [`merge_effect_conditions`].
    MergeEffectCondition,
    /// Merges an erase effect and a later assignment into a single durative effect. See [`merge_erase_assign_effects`].
    MergeEraseAssign,
    /// Annotates conditions and effects forming transitions and borrows. See [`annotate_transitions`].
    AnnotateTransitions,
}

/// Applies a built-in transformation to the model.
pub fn apply_transformation(mut model: Model, transformation: ModelTransformation) -> Res<Model> {
    match transformation {
        ModelTransformation::SplitPredicates => {
            split_predicates(&mut model)?;
            Ok(model)
        }
        ModelTransformation::LiftPredicates => {
            lift_predicates_to_state_functions(&mut model)?;
            Ok(model)
        }
        ModelTransformation::Intify => convert_reals_to_int(&model),
        ModelTransformation::EraseUnusableEffects => {
            erase_unusable_effects(&mut model)?;
            Ok(model)
        }
        ModelTransformation::MergeConditions => {
            merge_conditions(&mut model)?;
            Ok(model)
        }
        ModelTransformation::MergeEffectCondition => {
            merge_effect_conditions(&mut model)?;
            Ok(model)
        }
        ModelTransformation::MergeEraseAssign => {
            merge_erase_assign_effects(&mut model)?;
            Ok(model)
        }
        ModelTransformation::AnnotateTransitions => {
            annotate_transitions(&mut model)?;
            Ok(model)
        }
    }
}

/// Applies a sequence of built-in transformations to the model.
pub fn apply_transformations(mut model: Model, transformations: &[ModelTransformation]) -> Res<Model> {
    for tr in transformations {
        model = apply_transformation(model, *tr)?;
    }
    Ok(model)
}
