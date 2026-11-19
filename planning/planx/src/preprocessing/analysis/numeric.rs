//! Direction in which numeric fluents are changed by actions, and impact of these changes on requirements and metrics.

use std::collections::HashMap;

use crate::*;

use super::{ExprContext, NumericBounds, children, expression_roots};

/// Sign of the derivative of an expression with respect to a fluent.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum Sign {
    Pos,
    Neg,
    Unknown,
}

impl Sign {
    fn flip(self) -> Sign {
        match self {
            Sign::Pos => Sign::Neg,
            Sign::Neg => Sign::Pos,
            Sign::Unknown => Sign::Unknown,
        }
    }
}

/// For each numeric fluent, whether reads of the fluent favour larger values (`Pos`), smaller values (`Neg`),
/// or both/unknown.
struct Reads<'a> {
    env: &'a Environment,
    bounds: &'a NumericBounds<'a>,
    /// For each fluent, (some read favours larger values, some read favours smaller values).
    reads: HashMap<FluentId, (bool, bool)>,
}

impl Reads<'_> {
    fn record(&mut self, fluent: FluentId, sign: Sign) {
        let entry = self.reads.entry(fluent).or_default();
        match sign {
            Sign::Pos => entry.0 = true,
            Sign::Neg => entry.1 = true,
            Sign::Unknown => *entry = (true, true),
        }
    }

    /// Records all fluents of the expression as read in an unknown direction.
    fn unknown(&mut self, e: ExprId) {
        self.numeric(e, Sign::Unknown)
    }

    /// Visits a numeric expression, where larger values of the expression are favoured if `sign` is `Pos`.
    fn numeric(&mut self, e: ExprId, sign: Sign) {
        match self.env.node(e).expr() {
            Expr::StateVariable(f, args) => {
                self.record(*f, sign);
                for &a in args {
                    self.unknown(a);
                }
            }
            Expr::App(Fun::Plus, args) => args.iter().for_each(|&a| self.numeric(a, sign)),
            Expr::App(Fun::Minus, args) if args.len() == 1 => self.numeric(args[0], sign.flip()),
            Expr::App(Fun::Minus, args) if args.len() == 2 => {
                self.numeric(args[0], sign);
                self.numeric(args[1], sign.flip());
            }
            Expr::App(Fun::Mul, args) if args.len() == 2 => {
                for (x, y) in [(args[0], args[1]), (args[1], args[0])] {
                    let b = self.bounds.bounds(y);
                    let s = match (b.lb, b.ub) {
                        (Some(lb), _) if lb >= RealValue::ZERO => sign,
                        (_, Some(ub)) if ub <= RealValue::ZERO => sign.flip(),
                        _ => Sign::Unknown,
                    };
                    self.numeric(x, s);
                }
            }
            _ => children(e, self.env).into_iter().for_each(|c| self.unknown(c)),
        }
    }

    /// Visits a boolean expression that is required to be true if `positive` (false otherwise).
    fn requirement(&mut self, e: ExprId, positive: bool) {
        let s = if positive { Sign::Pos } else { Sign::Neg };
        match self.env.node(e).expr() {
            Expr::App(Fun::And | Fun::Or, args) => args.iter().for_each(|&a| self.requirement(a, positive)),
            Expr::App(Fun::Not, args) if args.len() == 1 => self.requirement(args[0], !positive),
            Expr::App(Fun::Implies, args) if args.len() == 2 => {
                self.requirement(args[0], !positive);
                self.requirement(args[1], positive);
            }
            Expr::Exists(_, e) | Expr::Forall(_, e) => self.requirement(*e, positive),
            Expr::App(Fun::Geq | Fun::Gt, args) if args.len() == 2 => {
                self.numeric(args[0], s);
                self.numeric(args[1], s.flip());
            }
            Expr::App(Fun::Leq | Fun::Lt, args) if args.len() == 2 => {
                self.numeric(args[0], s.flip());
                self.numeric(args[1], s);
            }
            _ => self.unknown(e),
        }
    }
}

/// Computes the [`NumericProperties`] of all numeric fluents of the model.
pub fn numeric_properties(model: &Model) -> HashMap<FluentId, NumericProperties> {
    let env = &model.env;
    let bounds = NumericBounds::new(model);
    // reads in the metric (whose root has the context `Other`) are oriented by the optimization direction
    let mut metric_reads = Reads {
        env,
        bounds: &bounds,
        reads: HashMap::new(),
    };
    for m in &model.metrics {
        match m {
            Metric::Minimize(e) => metric_reads.numeric(*e, Sign::Neg),
            Metric::Maximize(e) => metric_reads.numeric(*e, Sign::Pos),
        }
    }
    let mut other_reads = Reads {
        env,
        bounds: &bounds,
        reads: HashMap::new(),
    };
    for (root, ctx) in expression_roots(model) {
        let is_metric = model
            .metrics
            .iter()
            .any(|m| matches!(m, Metric::Minimize(e) | Metric::Maximize(e) if *e == root));
        match ctx {
            ExprContext::Requirement => other_reads.requirement(root, true),
            ExprContext::Other if is_metric => {}
            ExprContext::Other => other_reads.unknown(root),
        }
    }
    for (f, (pos, neg)) in metric_reads.reads {
        let entry = other_reads.reads.entry(f).or_default();
        entry.0 |= pos;
        entry.1 |= neg;
    }

    // directions of the changes made by actions
    let mut changes: HashMap<FluentId, (bool, bool)> = HashMap::new();
    for eff in model.actions.iter().flat_map(|a| &a.effects) {
        let e = &eff.effect_expression;
        let delta = |d: ExprId, s: Sign| {
            let b = bounds.bounds(d);
            let s = match (b.lb, b.ub) {
                (Some(lb), _) if lb >= RealValue::ZERO => s,
                (_, Some(ub)) if ub <= RealValue::ZERO => s.flip(),
                _ => Sign::Unknown,
            };
            match s {
                Sign::Pos => (true, false),
                Sign::Neg => (false, true),
                Sign::Unknown => (true, true),
            }
        };
        let (inc, dec) = match e.operation {
            EffectOp::Increase(d) => delta(d, Sign::Pos),
            EffectOp::Decrease(d) => delta(d, Sign::Neg),
            EffectOp::Assign(_) | EffectOp::Erase => (true, true),
        };
        let entry = changes.entry(e.state_variable.fluent).or_default();
        entry.0 |= inc;
        entry.1 |= dec;
    }

    let mut res = HashMap::new();
    for (id, fluent) in env.fluents.iter_with_id() {
        if !fluent.return_type.is_numeric() {
            continue;
        }
        let (increase, decrease) = match other_reads.reads.get(&id).copied().unwrap_or_default() {
            (false, false) => (DirectionImpact::Neutral, DirectionImpact::Neutral),
            (true, false) => (DirectionImpact::Helpful, DirectionImpact::Harmful),
            (false, true) => (DirectionImpact::Harmful, DirectionImpact::Helpful),
            (true, true) => (DirectionImpact::Mixed, DirectionImpact::Mixed),
        };
        let (inc, dec) = changes.get(&id).copied().unwrap_or_default();
        res.insert(
            id,
            NumericProperties {
                increase,
                decrease,
                never_increased: !inc,
                never_decreased: !dec,
            },
        );
    }
    res
}
