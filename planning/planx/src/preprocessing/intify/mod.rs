use std::collections::BTreeMap;

use num_integer::lcm;
use smallvec::smallvec;

use crate::*;

type SvPrecisions = RequiredScaling;
type Scaling = u64;

#[derive(Debug, Copy, Clone)]
struct ReachableValues {
    numer_lcm: Option<u64>,
    denom_lcm: u64,
}
impl ReachableValues {
    pub const INTEGER: ReachableValues = ReachableValues {
        numer_lcm: None,
        denom_lcm: 1,
    };
    pub fn from_constant(value: RealValue) -> Self {
        Self {
            numer_lcm: Some(value.numer().unsigned_abs()),
            denom_lcm: value.denom().unsigned_abs(),
        }
    }

    pub fn union(&mut self, other: ReachableValues) {
        self.numer_lcm = match (self.numer_lcm, other.numer_lcm) {
            (Some(x), Some(y)) if x.next_power_of_two() + y.next_power_of_two() < 20 => Some(lcm(x, y)),
            _ => None,
        };
        self.denom_lcm = lcm(self.denom_lcm, other.denom_lcm);
    }
    pub fn combine_additive(&mut self, other: ReachableValues) {
        // we give up finding out what are the possible values there
        self.numer_lcm = None;
        self.denom_lcm = lcm(self.denom_lcm, other.denom_lcm);
    }
    pub fn compbine_mul(&mut self, other: ReachableValues) {
        self.numer_lcm = match (self.numer_lcm, other.numer_lcm) {
            (Some(x), Some(y)) if x.next_power_of_two() + y.next_power_of_two() < 20 => Some(x * y),
            _ => None,
        };
        self.denom_lcm *= other.denom_lcm;
    }
    fn invert(&self) -> Option<Self> {
        self.numer_lcm.map(|self_numer_lcm| Self {
            numer_lcm: Some(self.denom_lcm),
            denom_lcm: self_numer_lcm,
        })
    }
}
impl Default for ReachableValues {
    fn default() -> Self {
        Self {
            numer_lcm: Some(1),
            denom_lcm: 1,
        }
    }
}
impl From<RealValue> for ReachableValues {
    fn from(value: RealValue) -> Self {
        Self::from_constant(value)
    }
}

struct RequiredScaling {
    fluents: BTreeMap<FluentId, ReachableValues>,
    time: ReachableValues,
}
impl RequiredScaling {
    fn insert(&mut self, fluent: FluentId) {
        assert!(!self.fluents.contains_key(&fluent));
        self.fluents.insert(fluent, Default::default());
    }

    fn add_possible_assignment(&mut self, fluent: FluentId, value: impl Into<ReachableValues>) {
        self.fluents.get_mut(&fluent).unwrap().union(value.into());
    }
    fn add_possible_increase(&mut self, fluent: FluentId, value: impl Into<ReachableValues>) {
        self.fluents.get_mut(&fluent).unwrap().combine_additive(value.into());
    }

    fn fluent_precisions(&self) -> impl Iterator<Item = (FluentId, Scaling)> {
        self.fluents.iter().map(|(f, s)| (*f, s.denom_lcm))
    }

    fn get(&self, fluent: FluentId) -> Option<ReachableValues> {
        self.fluents.get(&fluent).copied()
    }

    fn precision_of_fluent(&self, fluent: FluentId) -> Option<u64> {
        self.fluents.get(&fluent).map(|s| s.denom_lcm)
    }
}
impl Default for RequiredScaling {
    fn default() -> Self {
        Self {
            fluents: Default::default(),
            time: ReachableValues::default(),
        }
    }
}

pub fn convert_reals_to_int(model: &Model) -> Res<Model> {
    println!("Converting...");

    let mut scalings: SvPrecisions = Default::default();
    for (fid, f) in model.env.fluents.iter_with_id() {
        match f.return_type {
            Type::Real | Type::Int(_) => {
                scalings.insert(fid);
            }
            _ => continue,
        }
        println!("  {f:?}");
    }

    for eff in &model.init {
        update_precision_required_by_effect(eff, model, &mut scalings)?;
    }
    for act in model.actions.iter() {
        for eff in &act.effects {
            update_precision_required_by_effect(eff, model, &mut scalings)?;
        }
    }
    for (fid, prec) in scalings.fluent_precisions() {
        println!("{} : {}", model.env.fluents.get(fid).name(), prec);
    }

    let int_model = build_new_model(model, &scalings)?;
    println!("{}", int_model);

    Ok(int_model)
}

fn build_new_model(base: &Model, scaling_factors: &SvPrecisions) -> Res<Model> {
    let mut model = base.clone();
    for eff in &model.init {
        apply_scaling_to_effect(eff, &mut model.env, scaling_factors)?;
    }
    for act in model.actions.iter() {
        for eff in &act.effects {
            apply_scaling_to_effect(eff, &mut model.env, scaling_factors)?;
        }
    }

    for e in get_expr_to_scale(&model) {
        normalize(e, &mut model.env, scaling_factors)?;
    }

    Ok(model)
}

fn get_expr_to_scale(model: &Model) -> Vec<ExprId> {
    let mut exprs = Vec::with_capacity(64);
    for g in &model.goals {
        match g.goal_expression {
            SimpleGoal::HoldsDuring(_, expr_id, ..)
            | SimpleGoal::SometimeDuring(_, expr_id, ..)
            | SimpleGoal::AtMostOnceDuring(_, expr_id, ..) => exprs.push(expr_id),
            SimpleGoal::SometimeBefore { when, then }
            | SimpleGoal::SometimeAfter { when, then }
            | SimpleGoal::AlwaysWithin { when, then, .. } => {
                exprs.push(when);
                exprs.push(then);
            }
        }
    }
    for a in model.actions.iter() {
        for c in &a.conditions {
            exprs.push(c.cond);
        }
    }
    exprs
}

fn update_precision_required_by_effect(eff: &Effect, model: &Model, scalings: &mut SvPrecisions) -> Res<()> {
    // quantification is irrelevant of precision analysis, so just ignore it
    let eff = &eff.effect_expression;
    let fluent = eff.state_variable.fluent;
    if model.env.fluents.get(fluent).return_type.is_numeric() {
        match &eff.operation {
            EffectOp::Assign(expr_id) => {
                let prec = precision_of_expression(*expr_id, &model.env, scalings)?;
                scalings.add_possible_assignment(fluent, prec);
            }
            EffectOp::Increase(expr_id) | EffectOp::Decrease(expr_id) => {
                let prec = precision_of_expression(*expr_id, &model.env, scalings)?;
                scalings.add_possible_increase(fluent, prec);
            }
            EffectOp::Erase => {
                // effect does not update the fluent's value and thus does not impose any requirement on precision
            }
        }
    }

    Ok(())
}

fn precision_of_expression(expr_id: ExprId, env: &Environment, scalings: &SvPrecisions) -> Res<ReachableValues> {
    use Fun::*;
    let expr = env.node(expr_id);
    match expr.expr() {
        Expr::Real(ratio) => Ok(ReachableValues::from_constant(*ratio)),
        Expr::StateVariable(fluent_id, _) => scalings.get(*fluent_id).ok_or(expr.invalid("Not numeric?")),
        Expr::ViolationCount(_) => Ok(ReachableValues::INTEGER),
        Expr::App(Mul, args) => {
            let mut acc = ReachableValues::default();
            for &arg in args {
                acc.compbine_mul(precision_of_expression(arg, env, scalings)?);
            }
            Ok(acc)
        }
        Expr::App(Plus | Minus, args) => {
            let mut acc = ReachableValues::default();
            for &arg in args {
                acc.combine_additive(precision_of_expression(arg, env, scalings)?);
            }
            Ok(acc)
        }

        Expr::App(Div, args) => {
            let &[numer, denom] = args.as_array().unwrap();

            let mut numer = precision_of_expression(numer, env, scalings)?;
            let denom = precision_of_expression(denom, env, scalings)?;
            let inverted_denom = denom
                .invert()
                .ok_or_else(|| expr.invalid("could not sufficiently characterize the numerator of this expression"))?;
            numer.compbine_mul(inverted_denom);
            Ok(numer)
        }
        Expr::App(And | Or | Not | Implies | Eq | Leq | Geq | Lt | Gt, _)
        | Expr::Bool(_)
        | Expr::Object(_)
        | Expr::Param(_)
        | Expr::Exists(_, _)
        | Expr::Forall(_, _) => Err(expr.invalid("Non numeric expression when computing precision")),
        Expr::Instant(_) | Expr::Duration | Expr::Makespan => Ok(scalings.time),
    }
}

fn apply_scaling_to_effect(eff: &Effect, env: &mut Environment, scalings: &SvPrecisions) -> Res<()> {
    // TODO: scale conditions as well
    let eff = &eff.effect_expression;
    let fluent = eff.state_variable.fluent;
    if let Some(scaling_factor) = scalings.precision_of_fluent(fluent)
        && scaling_factor != 1
    {
        match &eff.operation {
            EffectOp::Assign(expr_id) | EffectOp::Increase(expr_id) | EffectOp::Decrease(expr_id) => {
                apply_scaling(*expr_id, scaling_factor, env, scalings)?;
            }
            EffectOp::Erase => {
                // effect does not update the fluent's value and thus does not impose any requirement on precision
            }
        }
    }
    Ok(())
}

fn apply_scaling(expr_id: ExprId, scaling_factor: u64, env: &mut Environment, scalings: &SvPrecisions) -> Res<()> {
    use Fun::*;
    let expr = env.node(expr_id);
    let replacement: Option<Expr> = match expr.expr() {
        Expr::Real(ratio) => Some(Expr::Real(*ratio * scaling_factor as i64)),
        Expr::StateVariable(fluent_id, _) => {
            let fluent_factor = scalings
                .precision_of_fluent(*fluent_id)
                .ok_or(expr.invalid("Not numeric?"))?;
            assert!(scaling_factor.is_multiple_of(fluent_factor));
            let remaining_scaling_factor = scaling_factor / fluent_factor;
            if remaining_scaling_factor != 1 {
                let new_node = expr.expr().clone();
                let span = expr.span().cloned();
                let dup = env.intern(new_node, span)?;
                let factor_node = env.intern(
                    Expr::Real(RealValue::from_integer(remaining_scaling_factor as i64)),
                    None,
                )?;
                Some(Expr::App(Mul, smallvec![factor_node, dup]))
            } else {
                None
            }
        }
        Expr::ViolationCount(_) => todo!(),
        Expr::App(Mul, args) => {
            let args = args.clone();
            let span = expr.span().cloned();
            let mut remaining_scaling_to_apply = scaling_factor;
            // multiply each element by its required precision
            for &arg in &args {
                let prec = precision_of_expression(arg, env, scalings)?.denom_lcm;
                assert!(remaining_scaling_to_apply.is_multiple_of(prec));
                remaining_scaling_to_apply /= prec;
                apply_scaling(arg, prec, env, scalings)?;
            }
            if remaining_scaling_to_apply != 1 {
                let constant = env.intern(
                    Expr::Real(RealValue::from_integer(remaining_scaling_to_apply as i64)),
                    None,
                )?;
                let previous = env.intern(Expr::App(Mul, args), span)?;
                Some(Expr::App(Mul, smallvec![constant, previous]))
            } else {
                None
            }
        }
        Expr::App(Plus | Minus, args) => {
            let args = args.clone();
            for arg in args {
                apply_scaling(arg, scaling_factor, env, scalings)?;
            }
            None
        }
        Expr::App(Div, args) => {
            let &[numer, _denom] = args.as_array().unwrap();
            // send everything to the numerator
            apply_scaling(numer, scaling_factor, env, scalings)?;
            None
        }
        Expr::App(And | Or | Not | Implies | Eq | Leq | Geq | Lt | Gt, _)
        | Expr::Bool(_)
        | Expr::Object(_)
        | Expr::Param(_)
        | Expr::Exists(_, _)
        | Expr::Forall(_, _) => {
            return Err(expr.invalid("Non numeric expression when applying scaling"));
        }
        Expr::Instant(_) => todo!(), // use time precision?
        Expr::Duration => todo!(),
        Expr::Makespan => todo!(),
    };
    if let Some(replacement) = replacement {
        env.replace(expr_id, replacement, None)?;
    }
    Ok(())
}

fn normalize(expr_id: ExprId, env: &mut Environment, scalings: &SvPrecisions) -> Res<()> {
    use Fun::*;
    let expr = env.node(expr_id);
    let replacement: Option<Expr> = match expr.expr() {
        Expr::StateVariable(_, _)
        | Expr::ViolationCount(_)
        | Expr::Real(_)
        | Expr::Bool(_)
        | Expr::Object(_)
        | Expr::Param(_) => None,
        Expr::App(Lt | Gt | Geq | Leq | Eq, args) => {
            let &[x, y] = args.as_array().unwrap();
            if env.node(x).tpe().is_numeric() {
                let mut prec_x = precision_of_expression(x, env, scalings)?;
                let prec_y = precision_of_expression(y, env, scalings)?;
                prec_x.combine_additive(prec_y);
                let required_scaling = prec_x.denom_lcm;
                for arg in [x, y] {
                    apply_scaling(arg, required_scaling, env, scalings)?;
                }
            } else {
                for arg in [x, y] {
                    normalize(arg, env, scalings)?;
                }
            }
            None
        }
        Expr::App(Or | And | Not, args) => {
            for arg in args.clone() {
                normalize(arg, env, scalings)?;
            }
            None
        }
        Expr::App(fun, small_vec) => return Err(expr.todo("not implemented")),
        Expr::Exists(_, expr_id) | Expr::Forall(_, expr_id) => {
            normalize(*expr_id, env, scalings);
            None
        }
        Expr::Instant(_) | Expr::Duration | Expr::Makespan => None,
    };
    Ok(())
}
