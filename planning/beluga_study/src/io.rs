use std::{collections::HashMap, sync::Arc};

use anyhow::{Context, Error};

use aries::core::Lit;
use aries_planning::chronicles;
use aries_planning::chronicles::Ctx;
use serde_json;

use prost::Message;
use unified_planning as up;

use crate::explanation::{BelugaQuestion};

// - Accept / read input (.json file)
//   - "instance" part:
//     - Isolate it
//     - Feed it to the python script (parser & up problem converter)
//       - include python files in this very same directory ?
//         - problem is, the execution will need to use a python interpreter... with unified planning installed, on top of that
//     - Read the obtained up problem (back in python)
//   - "plan" part (a reference plan that is only indicated when the problem is feasible):
//     - Clone the problem (see previous point)
//     - Consider the plan as a set of constraints (variable instantiations) and add them the (cloned) problem
//   - "question" part:
//     - read type and params
//
//          Alternative for above: 
//
//
// - Computation
//   - Explain / answer question
//
// - Write output
//   - `NL_explanation.md`: *mandatory* natural language explanation. Can use bullet points & headings.
//     - /!\ /!\ important /!\ /!\: make them clear !.. (-> improve output style of python prototype)
//   - `raw_explanation.json`: *optional*.
//     - "use suitable structure that best fits their technique and approach"
//   - `visual_explanation.png`: *optional*
//     - "table" visualisation ?
//

// pub fn test(filepath: String) -> Result<(), Error> {
// 
//     let file = std::fs::File::open(filepath)?;
//     let json: serde_json::Map<String, serde_json::Value> = serde_json::from_reader(file)?;
// 
// 
//     let binding = serde_json::value::to_raw_value(json.get("instance").unwrap()).unwrap();
//     let json = binding.get().;
//     // let json = json.get("instance").unwrap();
// 
//     println!("{json:?}");
// 
//     Ok(())
// 
// }

#[derive(Clone)]
pub struct BelugaCtx {
    pub ctx: Arc<Ctx>,
    pub beluga_orig_chr_prez_map: Option<HashMap<Lit, String>>,
    pub beluga_orig_chr_prez_map_rev: Option<HashMap<Lit, String>>,
}


// FIXME: alternatively, input could be the already serialized up problem, and then the 2 remaining json 

// pub fn interpret_input(up_filepath: String, json_filepath: String) -> Result<(chronicles::Problem, BelugaQuestion), Error> {
pub fn interpret_input(up_filepath: String, json_filepath: String) -> Result<(up::Problem, BelugaQuestion), Error> {

    let problem = std::fs::read(up_filepath)?;
    let problem = up::Problem::decode(problem.as_slice())?;

    // TODO/FIXME (PLACEHOLDER)
    let question = BelugaQuestion::WhyInfeasible;

    Ok((problem, question))
}

pub fn write_output() -> Result<(), Error> {
    todo!()
}

//// fn read_problem(problem: &serde_json::Value) -> Result<up::Problem, Error> {
//fn read_problem(problem: Vec<u8>) -> Result<up::Problem, Error> {
//
//    // FIXME the contents of "instance" need to be raw... ? RawValue (special feature of serde_json) ?
//
//    let problem = std::fs::read(problem)?;
////    let problem = up::Problem::decode(problem.as_slice())?;
////    let problem = Arc::new(problem);
////
////    beluga_problem_to_chronicles(&problem)
////        .with_context(|| format!("In problem {}/{}", &problem.domain_name, &problem.problem_name))
//}

fn read_question(question: &serde_json::Value) -> Result<BelugaQuestion, Error> {
    todo!()
}
