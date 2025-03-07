use aries_explainability::explain as expl;
use aries_explainability::musmcs_enumeration as musmcs;

pub enum BelugaQuestion {
    /// Self-explanatory
    WhyInfeasible,
    /// Why is jig x loaded on rack A instead of another rack B ?
    RackChoice(RackChoiceParams),
    /// Why load jig B on rack D instead of loading jig C on rack A ?
    JigToRackOrder(JigToRackOrderParams),
    /// Why not load jig C on rack A before loading jig B on rack D ?
    JigToRackOrderWhyNot(JigToRackOrderWhyNotParams),
    /// How can I reduce the number of swaps?      
    ReduceNumberSwaps,
    /// What is the impact of removing rack A for maintenance ?
    RackRemovalImpact(RackRemovalImpactParams),
    /// How can I keep one rack empty all the time ?
    KeepEmpty,
}

// FIXME: Strings rather than usize ?

struct RackChoiceParams { jig_id: usize, rack_a: usize, rack_b: usize }
struct JigToRackOrderParams { jig_x_id: usize, jig_y_id: usize, rack_c: usize, rack_d: usize }
struct JigToRackOrderWhyNotParams { jig_x_id: usize, jig_y_id: usize, rack_c: usize, rack_d: usize }
struct RackRemovalImpactParams { rack_id: usize }

// The explanations are expected to:
// - Determine the feasibility of the alternative solution.
// - Give an insight into the consequences that the alternative solution would entail.
// - Present a comparison of the chosen plan with alternative scenarios that were considered but not selected.

// impl<Lbl: String> expl::Question<Lbl> for Question {
//     fn check_presuppositions(&mut self) -> Result<(), expl::PresuppositionStatusCause> {
//         match self {
//             Question::WhyInfeasible => todo!(),
//             Question::RackChoice(params) => todo!(),
//             Question::JigToRackOrder(params) => todo!(),
//             Question::JigToRackOrderWhyNot(params) => todo!(),
//             Question::ReduceNumberSwaps => todo!(),
//             Question::RackRemovalImpact(params) => todo!(),
//             Question::KeepEmpty => todo!(),
//         }
//     }
// 
//     fn compute_explanation(&mut self) -> expl::Explanation<Lbl> {
//         match self {
//             Question::WhyInfeasible => todo!(),
//             Question::RackChoice(params) => todo!(),
//             Question::JigToRackOrder(params) => todo!(),
//             Question::JigToRackOrderWhyNot(params) => todo!(),
//             Question::ReduceNumberSwaps => todo!(),
//             Question::RackRemovalImpact(params) => todo!(),
//             Question::KeepEmpty => todo!(),
//         }
//     }
// }