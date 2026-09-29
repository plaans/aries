use crate::{
    core::{IntCst, LongCst, Var, cst_int_to_long},
    lang::linear::ScaledVar,
};

pub(super) type AuxVarTag = usize;

/// Either a "main" (i.e. non-auxiliary) or "auxiliary" x variable.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum XVar {
    /// Main (non-auxiliary): a variable from the aries solver.
    Main(Var),
    /// Auxiliary: a variable having no direct correspondance in the aries solver.
    /// However, it can be subject of half-bindings from the aries solver
    /// (when some literal is derived in aries, a bound of this auxiliary variable can be updated).
    Aux {
        tag: AuxVarTag,
        init_bounds: (IntCst, IntCst),
    },
}

/// A term of the form `a * X` where `X` is a [`XVar`] and `a` is a integer constant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ScaledXVar {
    Main(ScaledVar),
    Aux {
        tag: AuxVarTag,
        factor: IntCst,
        init_bounds_unscaled: (IntCst, IntCst),
    },
}
impl ScaledXVar {
    pub fn opp(&self) -> Self {
        match self {
            ScaledXVar::Main(svar) => Self::Main(ScaledVar {
                var: svar.var,
                factor: -svar.factor,
            }),
            &ScaledXVar::Aux {
                tag,
                factor,
                init_bounds_unscaled,
            } => ScaledXVar::Aux {
                tag,
                factor: -factor,
                init_bounds_unscaled,
            },
        }
    }
    pub fn upper_bound_long(&self, dom: impl crate::core::views::Dom) -> LongCst {
        match self {
            ScaledXVar::Main(svar) => svar.upper_bound_long(dom),
            &ScaledXVar::Aux {
                factor,
                init_bounds_unscaled,
                ..
            } => {
                let factor = cst_int_to_long(factor);
                // A negative factor swaps the two bounds of the variable, hence the `max`.
                LongCst::max(
                    cst_int_to_long(init_bounds_unscaled.0).saturating_mul(factor),
                    cst_int_to_long(init_bounds_unscaled.1).saturating_mul(factor),
                )
            }
        }
    }
    pub fn lower_bound_long(&self, dom: impl crate::core::views::Dom) -> LongCst {
        match self {
            ScaledXVar::Main(svar) => svar.lower_bound_long(dom),
            // A negative factor swaps the two bounds of the variable, hence the `min`.
            &ScaledXVar::Aux {
                factor,
                init_bounds_unscaled,
                ..
            } => {
                let factor = cst_int_to_long(factor);
                // A negative factor swaps the two bounds of the variable, hence the `min`.
                LongCst::min(
                    cst_int_to_long(init_bounds_unscaled.0).saturating_mul(factor),
                    cst_int_to_long(init_bounds_unscaled.1).saturating_mul(factor),
                )
            }
        }
    }
}

/// Returns a linear sum which is the opposite in terms of coefficient that the one given
///
/// We use it to detect that 2 constraints could use the same s variable in minilp
pub(super) fn get_opposite_scaled_xvar_sum(sum: &[ScaledXVar]) -> Vec<ScaledXVar> {
    let mut opp = Vec::new();
    for &sxvar in sum {
        opp.push(sxvar.opp());
    }
    opp
}

pub(super) fn simplify_scaled_xvar_sum(
    sum: (impl Into<Vec<ScaledXVar>>, IntCst),
) -> (impl Into<Vec<ScaledXVar>>, IntCst) {
    let (mut sum_terms, mut sum_cst) = (sum.0.into(), sum.1);

    sum_terms.sort_unstable_by_key(|sxv| *sxv);
    sum_terms.dedup_by(|second, first| match (first, second) {
        (ScaledXVar::Main(svar1), ScaledXVar::Main(svar2)) if svar1.var == svar2.var => {
            svar1.factor += svar2.factor;
            true
        }
        (
            ScaledXVar::Aux {
                tag: tag1,
                factor: factor1,
                ..
            },
            ScaledXVar::Aux {
                tag: tag2,
                factor: factor2,
                ..
            },
        ) if tag1 == tag2 => {
            *factor1 += *factor2;
            true
        }
        _ => false,
    });
    sum_terms.retain(|sxv| match sxv {
        ScaledXVar::Main(sv) => {
            if sv.is_zero() {
                false
            } else if sv.var == Var::ONE {
                sum_cst += sv.factor;
                false
            } else {
                true
            }
        }
        ScaledXVar::Aux {
            factor,
            init_bounds_unscaled,
            ..
        } => {
            if *factor == 0 || (init_bounds_unscaled.0 == 0 && init_bounds_unscaled.1 == 0) {
                false
            } else if init_bounds_unscaled.0 == 1 && init_bounds_unscaled.1 == 1 {
                sum_cst += factor;
                false
            } else {
                true
            }
        }
    });

    (sum_terms, sum_cst)
}
