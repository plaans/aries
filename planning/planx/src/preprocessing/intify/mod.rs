use std::{collections::BTreeMap, sync::Arc};

use itertools::Itertools;
use smallvec::smallvec;

use crate::{precision::NumericPrecision, preprocessing::Transformation, *};

type SvPrecisions = RequiredScaling;
type Scaling = u64;

type ReachableValues = NumericPrecision;

#[derive(Default)]
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
    fn add_possible_duration(&mut self, value: impl Into<ReachableValues>) {
        self.time.combine_additive(value.into());
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

    /// True if no required scaling is required for fluents or time.
    /// Some expression may still need it though (e.g. a goal `x <= 1.7`)
    fn fix_point(&self) -> bool {
        self.fluent_precisions().all(|(_fluent, scaling)| scaling == 1) && self.time.denom_lcm == 1
    }
}

pub fn convert_reals_to_int(model: &Model) -> Res<Model> {
    // println!("==== Original Model ====");
    // println!("{model}");
    println!("Converting reals to int...");

    let mut model = model.clone();

    for iter in 0.. {
        // compute by how much fluents need to be scaled to be integer
        // This initially assumes other fluents are integer, which is why we may need several rounds to get a fix point
        let scalings = compute_required_fluent_scalings(&model)?;

        if scalings.fix_point() && iter > 0 {
            // only break after a full path because reals may appear in e.g. goals and still need to be eliminated
            break;
        }
        for (fid, prec) in scalings.fluent_precisions() {
            if prec != 1 {
                println!("  {} *= {}", model.env.fluents.get(fid).name(), prec);
            }
        }
        println!("  time-scale *= {}", scalings.time.denom_lcm);

        if iter > 3 {
            return Message::error("Could not remove all non-integer values in model.").failed();
        }

        // build a new model with everything scaled
        model = build_new_model(model, &scalings)?;
    }

    // todo: we could update the type of all fluents to integer but are missing the API to do so

    let int_model = model;

    // println!("{}", int_model);

    Ok(int_model)
}

/// Looks at all values assigned/added to each numeric fluent to find out what values it can get.
fn compute_required_fluent_scalings(model: &Model) -> Res<SvPrecisions> {
    // initialize scaling to default value
    let mut scalings: SvPrecisions = Default::default();
    scalings.time.union(model.required_temporal_precision);
    for (fid, f) in model.env.fluents.iter_with_id() {
        match f.return_type {
            Type::Real | Type::Int(_) => {
                scalings.insert(fid);
            }
            _ => continue,
        }
    }

    // characterize the range of values that can be reached by fluents
    // Since a value only change through an effect (initial or in action) we only need
    // to characterize those
    for eff in &model.init {
        update_precision_required_by_effect(eff, model, &mut scalings)?;
    }
    for act in model.actions.iter() {
        for eff in &act.effects {
            update_precision_required_by_effect(eff, model, &mut scalings)?;
        }
        let durations: &[ExprId] = match act.duration {
            Duration::Fixed(dur) => &[dur],
            Duration::Bounded(lb, ub) => &[lb, ub],
            Duration::Instantaneous | Duration::Subtasks => &[],
        };
        for &dur in durations {
            let prec = precision_of_expression(dur, &model.env, &scalings)?;
            scalings.add_possible_duration(prec);
        }
    }
    Ok(scalings)
}

fn build_new_model(mut model: Model, scaling_factors: &SvPrecisions) -> Res<Model> {
    for eff in &mut model.init {
        apply_scaling_to_effect(eff, &mut model.env, scaling_factors)?;
    }
    for act in model.actions.iter_mut() {
        for eff in &mut act.effects {
            apply_scaling_to_effect(eff, &mut model.env, scaling_factors)?;
        }

        let durs: &[ExprId] = match act.duration {
            Duration::Instantaneous | Duration::Subtasks => &[],
            Duration::Fixed(dur) => &[dur],
            Duration::Bounded(lb, ub) => &[lb, ub],
        };
        for &dur in durs {
            apply_scaling(dur, scaling_factors.time.denom_lcm, &mut model.env, scaling_factors)?;
        }
    }

    for e in get_expr_to_scale(&model) {
        normalize(e, &mut model.env, scaling_factors)?;
    }
    // iterator over all goal expressions (from normal goals or in preferences)
    let goals_expressions = model
        .goals
        .iter_mut()
        .map(|g| &mut g.goal_expression)
        .chain(model.preferences.iter_mut().map(|p| &mut p.goal.goal_expression));
    for g in goals_expressions {
        match g {
            SimpleGoal::HoldsDuring(time_interval, ..)
            | SimpleGoal::SometimeDuring(time_interval, ..)
            | SimpleGoal::AtMostOnceDuring(time_interval, ..) => scale_interval(time_interval, scaling_factors),
            SimpleGoal::SometimeBefore { .. } | SimpleGoal::SometimeAfter { .. } => {}
            SimpleGoal::AlwaysWithin { delay, .. } => *delay *= scaling_factors.time.denom_lcm as i64,
        }
    }

    let mut objective_scalings = Vec::with_capacity(model.metrics.len());
    for metric in &model.metrics {
        let expr = match metric {
            Metric::Maximize(e) => *e,
            Metric::Minimize(e) => *e,
        };
        let prec = precision_of_expression(expr, &model.env, scaling_factors)?.denom_lcm;
        apply_scaling(expr, prec, &mut model.env, scaling_factors)?;
        objective_scalings.push(prec);
    }
    model.required_temporal_precision = NumericPrecision::INTEGER; // TODO: sanity check
    model.transformations.push(Arc::new(IntifyTransformation {
        objective_scalings,
        time_scaling: scaling_factors.time.denom_lcm,
    }));

    Ok(model)
}

fn scale_interval(itv: &mut TimeInterval, scalings: &SvPrecisions) {
    match itv {
        TimeInterval::Closed { start, end } | TimeInterval::Open { start, end } => {
            scale_time(start, scalings);
            scale_time(end, scalings);
        }
    }
}
fn scale_time(t: &mut Timestamp, scalings: &SvPrecisions) {
    t.delay *= scalings.time.denom_lcm as i64
}

/// Denotes the intification transformation (converting all reals to ints) that was applied.
struct IntifyTransformation {
    objective_scalings: Vec<u64>,
    time_scaling: u64,
}
impl Transformation for IntifyTransformation {
    fn describe(&self) -> String {
        format!(
            "Scaling reals into integers. Time: scaled by: {}, objectives scaled by: {}",
            self.time_scaling,
            self.objective_scalings.iter().format(" -- ")
        )
    }

    fn revert(&self, mut plan: SolutionPlan) -> Res<SolutionPlan> {
        if plan.metrics.len() != self.objective_scalings.len() {
            return Message::error(format!(
                "The plan has {} metrics but we expected {}",
                plan.metrics.len(),
                self.objective_scalings.len()
            ))
            .failed();
        }
        // reestablish the objectives to the values they had before the scaling was applied.
        for (metric, scaling) in plan.metrics.iter_mut().zip_eq(self.objective_scalings.iter()) {
            *metric /= *scaling as i64
        }
        for op in &mut plan.operations {
            op.start /= self.time_scaling as i64;
            op.duration /= self.time_scaling as i64;
        }
        Ok(plan)
    }

    fn apply(&self, mut plan: SolutionPlan) -> Res<SolutionPlan> {
        for (i, metric) in plan.metrics.iter_mut().enumerate() {
            let Some(scaling) = self.objective_scalings.get(i) else {
                return Message::error(format!("No scaling for the {i}-th metric.")).failed();
            };
            *metric *= *scaling as i64;
        }
        for op in &mut plan.operations {
            op.start *= self.time_scaling as i64;
            op.duration *= self.time_scaling as i64;
        }
        Ok(plan)
    }
}

/// Returns a collection of all top-level expressions in the model that we should scale.
fn get_expr_to_scale(model: &Model) -> Vec<ExprId> {
    let mut exprs = Vec::with_capacity(64);

    for g in &model.goals {
        match g.goal_expression {
            // FIXME: scale timings
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
        for e in &a.effects {
            e.effect_expression.condition.iter().for_each(|c| exprs.push(*c));
        }
    }
    exprs
}

/// Given an effect
fn update_precision_required_by_effect(eff: &Effect, model: &Model, scalings: &mut SvPrecisions) -> Res<()> {
    // quantification is irrelevant for precision analysis, so just ignore it
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
    scalings.add_possible_duration(eff.timing.delay);

    Ok(())
}

/// Given an arbitrary numeric expression determines its least value it should be multplied to to become integer.
fn precision_of_expression(expr_id: ExprId, env: &Environment, scalings: &SvPrecisions) -> Res<ReachableValues> {
    use Fun::*;
    let expr = env.node(expr_id);
    match expr.expr() {
        Expr::Real(ratio) => Ok(ReachableValues::from_constant(*ratio)),
        Expr::StateVariable(fluent_id, _) => scalings.get(*fluent_id).ok_or(expr.invalid("Not numeric?")),
        Expr::ViolationCount(_) | Expr::PlanLength => Ok(ReachableValues::INTEGER),
        Expr::App(Mul, args) => {
            let mut acc = ReachableValues::default();
            for &arg in args {
                acc.combine_mul(precision_of_expression(arg, env, scalings)?);
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
            numer.combine_mul(inverted_denom);
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

fn apply_scaling_to_effect(eff: &mut Effect, env: &mut Environment, scalings: &SvPrecisions) -> Res<()> {
    let eff = &mut eff.effect_expression;
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
    eff.timing.delay *= scalings.time.denom_lcm as i64;
    Ok(())
}

/// Applies the given scaling factor to the *numeric* expression.
///
/// The `scalings` parameter indicates the scaling already applied to each fluent.
fn apply_scaling(expr_id: ExprId, scaling_factor: u64, env: &mut Environment, scalings: &SvPrecisions) -> Res<()> {
    if scaling_factor == 1 {
        return Ok(());
    }
    use Fun::*;
    let expr = env.node(expr_id);
    // depending on the type of the expression,
    // 1) recursively apply scaling to the subexpressions
    // 2) return a new expression the should be substituted to `expr_id` (or None if we can keep the previous one.)
    let replacement: Option<Expr> = match expr.expr() {
        Expr::Real(ratio) => Some(Expr::Real(*ratio * scaling_factor as i64)),
        Expr::StateVariable(fluent_id, _) => {
            let fluent_factor = scalings
                .precision_of_fluent(*fluent_id)
                .ok_or(expr.invalid("Not numeric?"))?;
            assert!(scaling_factor.is_multiple_of(fluent_factor));
            // `expr` should be scaled by `scaling_factor`. Since it is already multiplied by
            // `fluent_factor`, lets compute but how much we still have to multiply it.
            let remaining_scaling_factor = scaling_factor / fluent_factor;
            if remaining_scaling_factor != 1 {
                // we still have to scale the expression,
                // create a new one that multplies the original one by the remaining scaling factor

                // duplicate the node (we will place something else at its ID)
                let new_node = expr.expr().clone();
                let span = expr.span().cloned();
                let dup = env.intern(new_node, span)?;

                // constant node with factor to apply
                let factor_node = env.intern(
                    Expr::Real(RealValue::from_integer(remaining_scaling_factor as i64)),
                    None,
                )?;

                // replace the current node by this one that multiplies it by the remaining scaling factor
                Some(Expr::App(Mul, smallvec![factor_node, dup]))
            } else {
                debug_assert_eq!(fluent_factor, scaling_factor);
                // the scaling entirely comes from the fluent, nothing to do
                None
            }
        }
        Expr::ViolationCount(_) => todo!(),
        Expr::PlanLength => todo!(),
        Expr::App(Mul, args) => {
            // for scaling a multiplication, we need to determine wy how much which should
            // scale each of its arguments
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
            // apply scaling uniformly to all arguments
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
        Expr::Instant(_) | Expr::Duration | Expr::Makespan => {
            let time_scaling = scalings.time.denom_lcm;
            assert!(scaling_factor.is_multiple_of(time_scaling));
            let remaining_scaling = scaling_factor / time_scaling;
            if remaining_scaling == 1 {
                None
            } else {
                return Err(expr.todo("should multiply!"));
            }
        }
    };
    if let Some(replacement) = replacement {
        env.replace(expr_id, replacement, None)?;
    }
    Ok(())
}

/// Normalizes a boolean expression by recursively applying scaling to all numeric sub-expressions
fn normalize(expr_id: ExprId, env: &mut Environment, scalings: &SvPrecisions) -> Res<()> {
    use Fun::*;
    let expr = env.node(expr_id);
    assert!(!expr.tpe().is_numeric());
    match expr.expr() {
        Expr::StateVariable(_, _) | Expr::Bool(_) | Expr::Object(_) | Expr::Param(_) => {
            // nothing to scale in these expressions
        }
        Expr::App(Lt | Gt | Geq | Leq | Eq, args) => {
            let &[lhs, rhs] = args.as_array().unwrap();
            if env.node(lhs).tpe().is_numeric() {
                // need to scale LHS and RHS by the same amount.
                // E.g. if lhs needs to be scale by 2 and and rhs by 3, we need to scale the both by 6
                // Determine the common scaling factor
                let precision_lhs = precision_of_expression(lhs, env, scalings)?;
                let precision_rhs = precision_of_expression(rhs, env, scalings)?;
                let mut combined_precision = precision_lhs;
                combined_precision.combine_additive(precision_rhs);

                // apply scaling to both term
                let required_scaling = combined_precision.denom_lcm;
                for arg in [lhs, rhs] {
                    apply_scaling(arg, required_scaling, env, scalings)?;
                }
            } else {
                // this is an equality between two symbolic parameters, apply normalization
                for arg in [lhs, rhs] {
                    normalize(arg, env, scalings)?;
                }
            }
        }
        Expr::App(Or | And | Not | Implies, args) => {
            for arg in args.clone() {
                normalize(arg, env, scalings)?;
            }
        }
        Expr::Exists(_, expr_id) | Expr::Forall(_, expr_id) => {
            normalize(*expr_id, env, scalings)?;
        }
        // numeric pattern on which this function should never be called
        Expr::Real(_) => unreachable!(),
        Expr::Instant(_) | Expr::Duration | Expr::Makespan => unreachable!(),
        Expr::App(Plus | Minus | Mul | Div, _) => unreachable!(),
        Expr::ViolationCount(_) | Expr::PlanLength => unreachable!(),
    };
    Ok(())
}
