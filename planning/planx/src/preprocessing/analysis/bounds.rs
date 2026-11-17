use std::collections::HashMap;

use crate::*;

use super::fluents_modified_by_actions;

/// Lower and upper bounds on a numeric value (`None` if unknown).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Bounds {
    pub lb: Option<RealValue>,
    pub ub: Option<RealValue>,
}

impl Bounds {
    pub const UNKNOWN: Bounds = Bounds { lb: None, ub: None };

    pub fn singleton(v: RealValue) -> Self {
        Bounds {
            lb: Some(v),
            ub: Some(v),
        }
    }

    fn neg(self) -> Self {
        Bounds {
            lb: self.ub.map(|x| -x),
            ub: self.lb.map(|x| -x),
        }
    }

    fn add(self, other: Self) -> Self {
        let add = |a: Option<RealValue>, b: Option<RealValue>| a?.checked_add(b?);
        Bounds {
            lb: add(self.lb, other.lb),
            ub: add(self.ub, other.ub),
        }
    }

    fn mul(self, other: Self) -> Self {
        let mul = |a: Option<RealValue>, b: Option<RealValue>| a?.checked_mul(b?);
        let zero = RealValue::ZERO;
        if let (Some(a), Some(b), Some(c), Some(d)) = (self.lb, self.ub, other.lb, other.ub) {
            let corners = [
                mul(Some(a), Some(c)),
                mul(Some(a), Some(d)),
                mul(Some(b), Some(c)),
                mul(Some(b), Some(d)),
            ];
            match corners.into_iter().collect::<Option<Vec<_>>>() {
                Some(corners) => Bounds {
                    lb: corners.iter().copied().min(),
                    ub: corners.iter().copied().max(),
                },
                None => Bounds::UNKNOWN,
            }
        } else if self.lb >= Some(zero) && other.lb >= Some(zero) {
            Bounds {
                lb: mul(self.lb, other.lb),
                ub: mul(self.ub, other.ub),
            }
        } else {
            Bounds::UNKNOWN
        }
    }

    fn div(self, denom: Self) -> Self {
        let zero = RealValue::ZERO;
        let one = RealValue::from_integer(1);
        let inv = |x: RealValue| one.checked_div(x);
        if denom.lb > Some(zero) {
            let inverse = Bounds {
                lb: denom.ub.map_or(Some(zero), inv),
                ub: inv(denom.lb.unwrap()),
            };
            self.mul(inverse)
        } else if matches!(denom.ub, Some(ub) if ub < zero) {
            self.neg().div(denom.neg())
        } else {
            Bounds::UNKNOWN
        }
    }
}

/// Bounds on the value of numeric expressions, derived from constants and from the initial values of static fluents.
///
/// The bounds hold for any evaluation of the expression in which all its state variables are defined.
pub struct NumericBounds<'a> {
    env: &'a Environment,
    pub(crate) dt: RealValue,
    /// Bounds on the values of static numeric fluents.
    fluents: HashMap<FluentId, Bounds>,
}

impl<'a> NumericBounds<'a> {
    pub fn new(model: &'a Model) -> Self {
        let modified = fluents_modified_by_actions(model);
        let mut values: HashMap<FluentId, Option<Vec<RealValue>>> = HashMap::new();
        for eff in &model.init {
            let e = &eff.effect_expression;
            let value = match e.operation {
                EffectOp::Assign(v)
                    if e.transition_start == Timestamp::ORIGIN
                        && e.condition.is_none()
                        && eff.universal_quantification.is_empty() =>
                {
                    match model.env.node(v).expr() {
                        Expr::Real(x) => Some(*x),
                        _ => None,
                    }
                }
                _ => None,
            };
            let entry = values.entry(e.state_variable.fluent).or_insert(Some(Vec::new()));
            match (value, entry.as_mut()) {
                (Some(v), Some(vs)) => vs.push(v),
                _ => *entry = None,
            }
        }
        let fluents = values
            .into_iter()
            .filter(|(f, _)| !modified.contains(f))
            .filter_map(|(f, vs)| {
                let vs = vs?;
                Some((
                    f,
                    Bounds {
                        lb: vs.iter().copied().min(),
                        ub: vs.iter().copied().max(),
                    },
                ))
            })
            .collect();
        NumericBounds {
            env: &model.env,
            dt: RealValue::new(1, model.required_temporal_precision.denom_lcm as i64),
            fluents,
        }
    }

    pub fn bounds(&self, expr: ExprId) -> Bounds {
        match self.env.node(expr).expr() {
            Expr::Real(x) => Bounds::singleton(*x),
            Expr::StateVariable(f, _) => self.fluents.get(f).copied().unwrap_or(Bounds::UNKNOWN),
            Expr::App(Fun::Plus, args) => (args.iter())
                .map(|&a| self.bounds(a))
                .fold(Bounds::singleton(RealValue::ZERO), Bounds::add),
            Expr::App(Fun::Minus, args) if args.len() == 1 => self.bounds(args[0]).neg(),
            Expr::App(Fun::Minus, args) if args.len() == 2 => self.bounds(args[0]).add(self.bounds(args[1]).neg()),
            Expr::App(Fun::Mul, args) => (args.iter())
                .map(|&a| self.bounds(a))
                .fold(Bounds::singleton(RealValue::from_integer(1)), Bounds::mul),
            Expr::App(Fun::Div, args) if args.len() == 2 => self.bounds(args[0]).div(self.bounds(args[1])),
            _ => Bounds::UNKNOWN,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(lb: i64, ub: i64) -> Bounds {
        Bounds {
            lb: Some(lb.into()),
            ub: Some(ub.into()),
        }
    }

    #[test]
    fn interval_arithmetic() {
        assert_eq!(b(1, 2).add(b(-3, 4)), b(-2, 6));
        assert_eq!(b(-1, 2).mul(b(-3, 4)), b(-6, 8));
        assert_eq!(b(1, 2).neg(), b(-2, -1));
        let half = RealValue::new(1, 2);
        assert_eq!(
            b(1, 4).div(b(-2, -1)),
            Bounds {
                lb: Some((-4).into()),
                ub: Some(-half)
            }
        );
        let positive = Bounds {
            lb: Some(2.into()),
            ub: None,
        };
        assert_eq!(
            b(1, 1).div(positive),
            Bounds {
                lb: Some(RealValue::ZERO),
                ub: Some(half)
            }
        );
        assert_eq!(b(1, 1).div(b(0, 2)), Bounds::UNKNOWN);
        assert_eq!(positive.mul(b(1, 3)), positive);
    }
}
