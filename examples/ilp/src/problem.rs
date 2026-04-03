use anyhow::*;
use aries::core::{INT_CST_MAX, INT_CST_MIN};
use aries::model::extensions::Shaped;
use aries::model::lang::linear::{LinearSum, LinearTerm};
use aries::prelude::*;
use lp_parser_rs::error::LpParseError;
use lp_parser_rs::lexer::{Lexer, ParseResult, RawConstraint};
use lp_parser_rs::lp::LpProblemParser;
use lp_parser_rs::model::Sense;
//use lp_parser_rs::mps::parse_mps;
use num_traits as num;
use std::collections::HashMap;

use crate::Model;

fn cast(val: f64) -> Result<IntCst> {
    if val >= INT_CST_MAX.into() {
        Ok(INT_CST_MAX)
    } else if val <= INT_CST_MIN.into() {
        Ok(INT_CST_MIN)
    } else if val.fract().abs() < 1e-6 {
        num::cast::<f64, IntCst>(val).ok_or_else(|| anyhow::anyhow!("{val} is not an integer (nor a binary) (1)."))
    } else {
        Err(anyhow::anyhow!("{val} is not an integer (nor a binary) (2)."))
    }
}

pub type ConstrHandle = (Vec<(String, IntCst)>, IntCst, IntCst);
#[derive(Debug)]
pub struct IlpProblem {
    pub vars: HashMap<String, (IntCst, IntCst)>,
    pub constrs: HashMap<String, ConstrHandle>,
    pub obj: Option<(String, Vec<(String, IntCst)>)>,
    pub sense: Sense,
}

impl IlpProblem {
    pub fn from_lp(input: &str) -> Result<Self> {
        let lexer = Lexer::new(input);
        let parser = LpProblemParser::new();
        let parse_result = parser.parse(lexer).map_err(LpParseError::from)?;
        Self::from_parse_result(parse_result)
    }

    /*pub fn from_mps(input: &str) -> Result<Self> {
        todo!("TODO FIXME BUG : broken parsing: considers 'EQ' for rhs instead of 'LTE'");
        let parse_result = parse_mps(input)?;
        Self::from_parse_result(parse_result)
    }*/

    fn from_parse_result(parse_result: ParseResult) -> Result<Self> {
        let mut vars = HashMap::new();
        let mut constrs = HashMap::new();
        let obj = {
            if let Some(obj) = parse_result.objectives.first() {
                let mut coefs = vec![];
                for coef in &obj.coefficients {
                    coefs.push((coef.name.to_string(), cast(coef.value)?))
                }
                Some((obj.name.to_string(), coefs))
            } else {
                None
            }
        };
        let sense = parse_result.sense;

        for var_name in &parse_result.binaries {
            vars.insert(var_name.to_string(), (0, 1));
        }
        for var_name in &parse_result.generals {
            vars.insert(var_name.to_string(), (0, INT_CST_MAX));
        }
        for var_name in &parse_result.integers {
            vars.insert(var_name.to_string(), (0, INT_CST_MAX));
        }
        //if !parse_result.integers.is_empty() {
        //    return Err(anyhow::anyhow!("'Integer' variables are unsupported, as there are different interpretations of their default bounds."));
        //}
        if !parse_result.semi_continuous.is_empty() {
            return Err(anyhow::anyhow!("Semi-continuous variables unsupported."));
        }
        if !parse_result.sos.is_empty() {
            return Err(anyhow::anyhow!("SOS constraints unsupported."));
        }
        for (var_name, bounds) in parse_result.bounds {
            let (lb, ub) = vars
                .get_mut(var_name)
                .ok_or_else(|| anyhow::anyhow!("Continuous variables unsupported."))?;
            match bounds {
                lp_parser_rs::model::VariableType::Free => {
                    *lb = INT_CST_MIN;
                    *ub = INT_CST_MAX;
                }
                lp_parser_rs::model::VariableType::General => {
                    *lb = 0;
                    *ub = INT_CST_MAX;
                }
                lp_parser_rs::model::VariableType::LowerBound(_lb) => {
                    *lb = cast(_lb)?;
                }
                lp_parser_rs::model::VariableType::UpperBound(_ub) => {
                    *ub = cast(_ub)?;
                }
                lp_parser_rs::model::VariableType::DoubleBound(_lb, _ub) => {
                    *lb = cast(_lb)?;
                    *ub = cast(_ub)?;
                }
                lp_parser_rs::model::VariableType::Binary => {
                    *lb = 0;
                    *ub = 1;
                }
                lp_parser_rs::model::VariableType::Integer => {
                    *lb = 0;
                    *ub = INT_CST_MAX;
                    //return Err(anyhow::anyhow!("'Integer' variables are unsupported, as there are different interpretations of their default bounds."));
                }
                lp_parser_rs::model::VariableType::SemiContinuous => unreachable!(),
                lp_parser_rs::model::VariableType::SOS => unreachable!(),
            }
        }

        for c in &parse_result.constraints {
            match c {
                RawConstraint::SOS { .. } => unreachable!(),
                RawConstraint::Standard {
                    name,
                    coefficients,
                    operator,
                    rhs,
                    ..
                } => {
                    let mut coefs = vec![];
                    for coef in coefficients {
                        coefs.push((coef.name.to_string(), cast(coef.value)?));
                    }
                    let (lb, ub) = match operator {
                        // ??FIXME??
                        lp_parser_rs::model::ComparisonOp::GT => (cast(*rhs)? + 1, INT_CST_MAX),
                        lp_parser_rs::model::ComparisonOp::GTE => (cast(*rhs)?, INT_CST_MAX),
                        lp_parser_rs::model::ComparisonOp::EQ => (cast(*rhs)?, cast(*rhs)?),
                        lp_parser_rs::model::ComparisonOp::LT => (INT_CST_MIN, cast(*rhs)? - 1),
                        lp_parser_rs::model::ComparisonOp::LTE => (INT_CST_MIN, cast(*rhs)?),
                    };
                    constrs.insert(name.to_string(), (coefs, lb, ub));
                }
            }
        }

        Ok(Self {
            vars,
            constrs,
            obj,
            sense,
        })
    }

    pub fn encode_model(&self) -> Result<Model> {
        let mut model = Model::new();

        for (var_name, (lb, ub)) in &self.vars {
            model.new_ivar(*lb, *ub, var_name.clone());
        }

        for (coefs, lb, ub) in self.constrs.values() {
            let sum = LinearSum::of(
                coefs
                    .iter()
                    .map(|(var_name, coef)| LinearTerm::int(*coef, model.get_int_var(var_name).unwrap()))
                    .collect(),
            );

            if *lb > INT_CST_MIN {
                model.enforce(sum.clone().geq(*lb), []);
            }
            if *ub < INT_CST_MAX {
                model.enforce(sum.clone().leq(*ub), []);
            }
            //println!("{}: {} <= {} <= {}", constr_name, lb, sum.clone(), ub);
        }

        if let Some((obj_name, coefs)) = &self.obj {
            let obj_atom = model.new_ivar(INT_CST_MIN, INT_CST_MAX, obj_name.clone());

            let sum = LinearSum::of(
                coefs
                    .iter()
                    .map(|(var_name, coef)| LinearTerm::int(*coef, model.get_int_var(var_name).unwrap()))
                    .collect(),
            );

            model.enforce(sum.clone().leq(obj_atom), []);
            model.enforce(sum.clone().geq(obj_atom), []);
        }

        Ok(model)
    }
}
