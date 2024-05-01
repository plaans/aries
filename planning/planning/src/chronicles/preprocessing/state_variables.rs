use crate::chronicles::analysis::is_static;
use crate::chronicles::constraints::Constraint;
use crate::chronicles::{Chronicle, Container, Effect, EffectOp, Fluent, Problem, StateVar, Time, VarType};
use aries::model::lang::*;
use aries::model::symbols::{SymId, TypedSym};
use aries::model::types::{TypeHierarchy, TypeId};
use aries::utils::input::Sym;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use tracing;

/// Substitute predicates into state functions where applicable.
/// For instance the predicate `(at agent location) -> boolean` can usually be
/// transformed into the state function `(at agent) -> location`.
/// For this transformation to be applicable, it should be the case that,
/// for a given `agent`, there is at most one `location` such that `(at agent location) = true`.
///
/// The process is inspired by the paper:
/// "The Extracting Mutual Exclusion Invariants from Lifted Temporal Planning Domains"
pub fn lift_predicate_to_state_variables(pb: &mut Problem) {
    // identify a set of candidate groups that may be substituted by state variables
    let mut candidates = find_compound_state_variables(pb);

    // sort the candidates by decreasing length
    // this gives higher priority to groups that contains more state variables
    // This is a heuristic choice as they are not necessarily better
    // An important side effect is that functional (static) state variables are processed last
    // (they contain at most one fluent) which is critical as processing groups of size 2 might
    // need them in their current form when processing
    candidates.sort_by_key(|c| c.fluents.len());
    candidates.reverse();

    // the same fluent may appear multiple times in different groups,
    // for these, only keep the first group.
    let mut fluents_to_remove = HashSet::new();
    let mut group_index = 0;
    while group_index < candidates.len() {
        let group = &candidates[group_index];
        if group.fluents.iter().any(|f| fluents_to_remove.contains(&f.fluent)) {
            candidates.remove(group_index);
        } else {
            fluents_to_remove.extend(group.fluents.iter().map(|f| f.fluent.clone()));
            group_index += 1;
        }
    }

    if !candidates.is_empty() {
        println!("Lifting predicates to state variables:")
    }
    for c in &candidates {
        println!("  - {:<40} [from: {c:?}]", c.name());
        lift(pb, c);
    }
}

/// A function is a fluent that is static and maps its input parameter to a single output
#[derive(Clone, PartialEq, Eq)]
struct Function {
    fluent: Arc<Fluent>,
    in_param: usize,
    out_param: usize,
}

impl Function {
    fn param(&self) -> Type {
        self.fluent.argument_types()[self.in_param]
    }
    fn result(&self) -> Type {
        self.fluent.argument_types()[self.out_param]
    }
}

fn find_compound_state_variables(pb: &Problem) -> Vec<SubstitutionGroup> {
    let mut valid_groups = Vec::new();

    let types = &pb.context.model.shape.symbols.types;
    let mut functions = Vec::new();

    for f in &pb.context.fluents {
        for candidate in SubstitutedFluent::candidates(f.clone(), &functions) {
            // tracing::trace!("candidate: {candidate:?}");
            let Some(group) = SubstitutionGroup::new(vec![candidate.clone()], types) else {
                // group is syntactically valid
                // tracing::trace!("  not a valid group");
                continue;
            };

            if is_substitutable(pb, &group) {
                // this group is substitutable by a single multi-valued state variable

                // check if it is a function (i.e. static) and mark it as such
                // this is useful as functions are also used in the later step when combining two predicates
                if is_static(f.as_ref(), pb) && f.argument_types().len() == 2 {
                    let out_param = candidate.lifted_param.unwrap();
                    let in_param = 1 - out_param;
                    let fun = Function {
                        fluent: f.clone(),
                        in_param,
                        out_param,
                    };
                    functions.push(fun);
                }
                valid_groups.push(group);
                break;
            }
        }
    }
    let mut candidates = Vec::new();
    for f in &pb.context.fluents {
        if !is_static(f.as_ref(), pb) {
            candidates.extend_from_slice(&SubstitutedFluent::candidates(f.clone(), &functions))
        }
    }

    // detect groups of 2 predicates
    for c1 in &candidates {
        for c2 in &candidates {
            if c1.fluent.sym <= c2.fluent.sym {
                continue;
            }

            if let Some(group) = SubstitutionGroup::new(vec![c1.clone(), c2.clone()], types) {
                if is_substitutable(pb, &group) {
                    valid_groups.push(group);
                }
            }
        }
    }

    // process groups of 3 predicates
    for c1 in &candidates {
        for c2 in &candidates {
            for c3 in &candidates {
                if c1.fluent.sym <= c2.fluent.sym || c2.fluent.sym <= c3.fluent.sym {
                    continue;
                }

                if let Some(group) = SubstitutionGroup::new(vec![c1.clone(), c2.clone(), c3.clone()], types) {
                    if is_substitutable(pb, &group) {
                        valid_groups.push(group);
                    }
                }
            }
        }
    }
    valid_groups
}

#[derive(Clone, PartialEq, Eq)]
struct FunctionApplication {
    /// fluent that must functional: f(x) = y
    function: Function,
    /// index of the parameter of the original fluent that must be given as parameter to the function
    in_param: usize,
}

impl Debug for FunctionApplication {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}({})", &self.function.fluent.name, self.in_param)
    }
}

#[derive(Clone)]
struct SubstitutedFluent {
    fluent: Arc<Fluent>,
    lifted_param: Option<usize>,
    functional_params: Vec<FunctionApplication>,
}

#[derive(Clone, Eq, PartialEq, Debug)]
enum Param {
    /// Parameter of the synthesized fluent is the i-th of the original fluent
    Original(usize),
    /// Parameter of the synthesized fluent is a function of the i-th parameter of the original fluent
    Synthesized(FunctionApplication),
}

impl SubstitutedFluent {
    /// Creates a new substituted fluent whose return value is one of its original parameters
    pub fn new(fluent: &Arc<Fluent>, lifted_param: usize) -> Self {
        SubstitutedFluent {
            fluent: fluent.clone(),
            lifted_param: Some(lifted_param),
            functional_params: Default::default(),
        }
    }
    /// Creates a new substituted fluent that keeps its boolean return value
    pub fn new_not_lifted(fluent: &Arc<Fluent>) -> Self {
        SubstitutedFluent {
            fluent: fluent.clone(),
            lifted_param: None,
            functional_params: Default::default(),
        }
    }
    fn args(&self) -> Vec<Type> {
        let mut args = self.fluent.argument_types().iter().copied().collect_vec();
        if let Some(lifted_param) = self.lifted_param {
            args.remove(lifted_param);
        }
        for app in &self.functional_params {
            args.push(app.function.result())
        }
        args
    }
    fn params(&self) -> Vec<(Param, Type)> {
        let mut args = self
            .fluent
            .argument_types()
            .iter()
            .enumerate()
            .filter_map(|(i, t)| {
                if Some(i) == self.lifted_param {
                    None
                } else {
                    Some((Param::Original(i), *t))
                }
            })
            .collect_vec();
        for app in &self.functional_params {
            args.push((Param::Synthesized(app.clone()), app.function.result()));
        }
        args
    }

    fn return_type(&self) -> Type {
        if let Some(lifted_param) = self.lifted_param {
            self.fluent.argument_types()[lifted_param]
        } else {
            self.fluent.return_type()
        }
    }

    /// Returns a list of potential substituted fluents
    fn candidates(f: Arc<Fluent>, functions: &[Function]) -> Vec<SubstitutedFluent> {
        let mut candidates: Vec<_> = (0..f.argument_types().len())
            .rev()
            .map(|i| SubstitutedFluent::new(&f, i))
            .collect();
        candidates.push(SubstitutedFluent::new_not_lifted(&f));

        let candidates_copy = candidates.clone();

        // consider augmentation of the parameter with functional parameters
        for fun in functions {
            for c in &candidates_copy {
                if c.return_type() == fun.param() {
                    // this function is applicable to the return type.
                    // its application could be used as an additional parameter (useful for unifying with other fluents)
                    // create a new version with an additional parameter
                    let mut augmented_c = c.clone();
                    let application_param = c.lifted_param.unwrap();
                    augmented_c.functional_params.push(FunctionApplication {
                        function: fun.clone(),
                        in_param: application_param,
                    });
                    candidates.push(augmented_c);
                }
            }
        }

        candidates
    }
}

impl Debug for SubstitutedFluent {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}(", &self.fluent.name)?;
        let mut first = true;
        for i in 0..self.fluent.argument_types().len() {
            if Some(i) != self.lifted_param {
                if !first {
                    write!(f, " ,")?;
                }
                first = false;
                write!(f, "{i}")?;
            }
        }
        for app in &self.functional_params {
            if !first {
                write!(f, " ,")?;
            }
            first = false;
            write!(f, "{app:?}")?;
        }
        match self.lifted_param {
            Some(i) => write!(f, ") -> {i}"),
            None => write!(f, ") -> Bool"),
        }
    }
}

struct Mapping {
    from_orig: Vec<Param>,
}

impl Mapping {
    fn new(trans: Vec<Param>) -> Self {
        Mapping { from_orig: trans }
    }
}

/// A group of fluent that are syntactically compatible to form a new multi-valued fluent
struct SubstitutionGroup {
    fluents: Vec<SubstitutedFluent>,
    args: Vec<Type>,
    args_mapping: Vec<Mapping>,
    return_type: TypeId,
}

impl Debug for SubstitutionGroup {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        for sf in &self.fluents {
            write!(f, "{sf:?}:")?
        }
        Ok(())
    }
}

impl SubstitutionGroup {
    /// Attempts to create a combination of the given fluents into a syntactically valid group.
    /// Return None if the fluents could not be unified
    fn new(fluents: Vec<SubstitutedFluent>, types: &TypeHierarchy) -> Option<Self> {
        tracing::trace!("{fluents:?}");
        if fluents.iter().any(|f| f.fluent.return_type() != Type::Bool) {
            tracing::trace!("  a fluent is not boolean");
            return None;
        }
        if fluents.is_empty() {
            tracing::trace!("  no fluent in group");
            return None;
        }
        if fluents.len() == 1 && fluents[0].lifted_param.is_none() {
            tracing::trace!("  no lifted param and single fluent");
            return None;
        }

        let args = fluents[0].args();
        let mut args_mapping: Vec<_> = Default::default();

        for fluent in &fluents {
            if fluent.args().len() != args.len() {
                tracing::trace!("  incorrect number of args");
                return None; // arguments are not compatible
            }

            let mut mapping = Vec::new();
            let fluent_args = fluent.params();
            for expected_type in &args {
                // we need to find a parameter of the fluent with the given type
                let mut found = false;
                for (param, tpe) in &fluent_args {
                    if tpe == expected_type && !mapping.contains(param) {
                        mapping.push(param.clone());
                        found = true;
                        break;
                    }
                }
                if !found {
                    tracing::trace!("  incompatible type of arguments {args:?}  {expected_type:?} {fluent_args:?}");
                    return None; // incompatible types of arguments
                }
            }
            args_mapping.push(Mapping::new(mapping))
        }

        let return_type = if fluents.len() == 1 {
            match fluents[0].return_type() {
                Type::Sym(tid) => tid,
                _ => unreachable!(),
            }
        } else {
            // valid if there are no two overlapping symbolic types
            let return_values: Vec<_> = fluents.iter().map(|f| f.return_type()).collect();
            for (i, ret1) in return_values.iter().enumerate() {
                match ret1 {
                    Type::Sym(t1) => {
                        // symbolic type for a fluent, all other symbolic types must have non overlapping values
                        for ret2 in &return_values[i + 1..] {
                            if let Type::Sym(t2) = ret2 {
                                if types.are_compatible(*t1, *t2) {
                                    // two return types have overlapping values.
                                    // unifying them means that we would not be able to distinguish them
                                    return None;
                                }
                            }
                        }
                    }
                    Type::Bool => {
                        // boolean types are compatible with everything as they would be substituted byt the symbol of the fluent
                    }
                    _ => return None, // numeric fluents cannot be lifted
                }
            }
            // combination of two symbolic types, we give the top type as the return value of the fluent.
            types.top_type()
        };

        Some(SubstitutionGroup {
            fluents,
            args,
            args_mapping,
            return_type,
        })
    }

    /// Returns true if the given fluent is part of the group
    pub fn contains(&self, f: &Fluent) -> bool {
        self.fluents.iter().any(|ff| ff.fluent.sym == f.sym)
    }

    fn name(&self) -> String {
        self.fluents.iter().map(|f| &f.fluent.name.canonical).join(":")
    }

    /// Returns the value that the fluent would have for the given state variable
    fn value_sv(&self, sv: &StateVar) -> SAtom {
        self.value(&sv.fluent, &sv.args)
    }

    fn value(&self, fluent: &Fluent, args: &[SAtom]) -> SAtom {
        let fid = self.fluents.iter().position(|sf| sf.fluent.as_ref() == fluent).unwrap();
        match self.fluents[fid].lifted_param {
            Some(i) => args[i],
            None => SAtom::Cst(TypedSym::new(fluent.sym, self.return_type)),
        }
    }

    /// Returns the parameters to be given to the synthesized state variable.
    /// Requires the chronicle where the state variable is used to find out which variable to use for functional parameters.
    /// May return None, if there is no condition binding a functional parameter to a variable.
    fn affected_state_variable(&self, fluent: &Fluent, args: &[SAtom], ctx: &Chronicle) -> Option<Vec<SAtom>> {
        let fid = self.fluents.iter().position(|sf| sf.fluent.as_ref() == fluent).unwrap();
        let mapping = &self.args_mapping[fid];
        let mut params = Vec::with_capacity(self.args.len());
        for p in &mapping.from_orig {
            let x = match p {
                Param::Original(i) => args[*i],
                Param::Synthesized(app) =>
                // the parameter is of the form f(x)
                // look for a condition that enforces y = f(x) and replace with y
                {
                    ctx.conditions
                        .iter()
                        .filter(|c| c.state_var.fluent == app.function.fluent)
                        .filter_map(|c| {
                            let in_arg = args[app.in_param];
                            if in_arg == c.state_var.args[app.function.in_param] {
                                Some(c.state_var.args[app.function.out_param])
                            } else {
                                None
                            }
                        })
                        .next()?
                }
            };
            params.push(x)
        }
        Some(params)
    }
    fn affected(&self, sv: &StateVar, ctx: &Chronicle) -> Option<Vec<SAtom>> {
        self.affected_state_variable(&sv.fluent, &sv.args, ctx)
    }
}

/// Checks whether a group is substitutable, meaning that of all predicate that may map to a single
/// ground state variable, only one may be true at any point in time

fn is_substitutable(pb: &Problem, group: &SubstitutionGroup) -> bool {
    let fluent_name = format!("{group:?}");
    let _span = tracing::span!(tracing::Level::TRACE, "to-sv", fluent = fluent_name).entered();
    let model = &pb.context.model;
    // only keep boolean state functions
    for sf in &group.fluents {
        if sf.fluent.return_type() != Type::Bool {
            tracing::trace!("not bool");
            return false;
        }
    }

    let on_target_fluent = |sv: &StateVar| group.fluents.iter().any(|e| e.fluent == sv.fluent);

    let mut assignments = HashSet::new();
    for ch in &pb.chronicles {
        // check that we don't have more than one positive effect
        for eff in ch.chronicle.effects.iter().filter(|e| on_target_fluent(&e.state_var)) {
            if let Some(e) = as_cst_eff(eff) {
                if e.value {
                    // positive assignment
                    // let (_, args, val) = e.into_assignment();
                    let Some(args) = group.affected_state_variable(&e.fluent, &eff.state_var.args, &ch.chronicle)
                    else {
                        tracing::trace!("could not identify variable for functional param");
                        return false;
                    };
                    if assignments.contains(&args) {
                        // more than one assignment, abort
                        tracing::trace!("more than one positive assignment in base chronicles");
                        return false;
                    } else {
                        assignments.insert(args);
                    }
                } else {
                    // negative assignment, not supported
                    tracing::trace!("negative assignment in base chronicle");
                    return false;
                }
            } else {
                tracing::trace!("possible static effect with non-constant?");
                // we have a possible static effect that contains non-constant, abort
                return false;
            }
        }

        // check that we have only positive conditions for this sv
        for cond in ch
            .chronicle
            .conditions
            .iter()
            .filter(|c| on_target_fluent(&c.state_var))
        {
            if model.unifiable(cond.value, false) {
                // note that it is assumed that if an effect is present, it may be needed by someone
                // (there a special preprocessing phase that removes provably unused statements)
                tracing::trace!("non positive condition in base");
                return false;
            }
        }
    }
    for template in &pb.templates {
        let mut has_negative_condition = false; // TODO: wrong place?
                                                // check that we have only conditions with constant value for this sv
        for cond in template
            .chronicle
            .conditions
            .iter()
            .filter(|c| on_target_fluent(&c.state_var))
        {
            match bool::try_from(cond.value) {
                Err(_) => {
                    tracing::trace!("non constant condition in template");
                    return false; // not a constant value
                }
                Ok(true) => {}
                Ok(false) => has_negative_condition = true, // having negative conditions can be restrictive in temporal models
            }
        }

        for (_, effects) in &template
            .chronicle
            .effects
            .iter()
            .filter(|e| on_target_fluent(&e.state_var))
            .group_by(|e| group.affected(&e.state_var, &template.chronicle))
        {
            let effects: Vec<_> = effects.collect();
            let positives = effects
                .iter()
                .filter(|e| e.operation == EffectOp::TRUE_ASSIGNMENT)
                .collect_vec();
            let negatives = effects
                .iter()
                .filter(|e| e.operation == EffectOp::FALSE_ASSIGNMENT)
                .collect_vec();
            // we must have exactly one positive and on negative effect
            if positives.len() != 1 || negatives.len() != 1 || effects.len() != 2 {
                tracing::trace!("not exactly one positive and one negative {:?}", template.label);
                return false;
            }
            let pos = positives[0];
            let neg = negatives[0];

            if group.affected(&pos.state_var, &template.chronicle)
                != group.affected(&neg.state_var, &template.chronicle)
            {
                tracing::trace!("not the same synthesized state variable");
                return false;
            }

            if pos.transition_end != neg.transition_end
                || pos.transition_start != neg.transition_start
                || pos.min_mutex_end != neg.min_mutex_end
            {
                // they do not cover exactly the same interval
                // in this case, we require that the positive comes after the negative
                // and that there is no negative conditions on the fluents (because they will be merged
                // into a single transition)
                if pos.transition_start != template.chronicle.end || neg.transition_start != template.chronicle.start {
                    dbg!(pos.transition_start, template.chronicle.end);
                    dbg!(neg.transition_start, template.chronicle.start);
                    tracing::trace!("unrecognized pattern for temporal split");
                    return false;
                }

                if has_negative_condition {
                    tracing::trace!("not covering the same interval and negative conditions");
                    return false;
                }
            }
        }
    }
    tracing::trace!("OK");
    // we have passed all the tests, this predicate can be lifted as a state variable
    true
}

/// Transform the problem so that all fluents of the group are translated into a single state variable
fn lift(pb: &mut Problem, group: &SubstitutionGroup) {
    // remove all fluents from the group
    pb.context
        .fluents
        .retain(|f| group.fluents.iter().all(|frm| &frm.fluent != f));

    let mut signature = group.args.clone();
    signature.push(Type::Sym(group.return_type));
    let fluent = Arc::new(Fluent {
        name: Sym::new(group.name()),
        sym: group.fluents[0].fluent.sym, // use symbol of the first fluent as we cannot create new symbols
        signature,
    });
    pb.context.fluents.push(fluent.clone());

    // returns true if the corresponding state variable is one to be substituted
    let must_substitute = |sv: &Fluent| group.contains(sv);

    let return_type = |sv: &Fluent| match sv.return_type() {
        Type::Sym(type_id) => type_id,
        _ => unreachable!(""),
    };

    let is_true_assignment = |eff: &EffectOp| eff == &EffectOp::TRUE_ASSIGNMENT;
    let is_false_assignment = |eff: &EffectOp| eff == &EffectOp::FALSE_ASSIGNMENT;

    let mut transform_chronicle = |ch: &mut Chronicle, container_label: Container| {
        let ch_copy = ch.clone();
        // record all variables created in the process, they will need to me added to the chronicles
        // parameters afterward
        let mut created_variables: Vec<Variable> = Default::default();
        for cond in &mut ch.conditions {
            if must_substitute(&cond.state_var.fluent) {
                // swap fluent with the lifted one
                let params = group.affected(&cond.state_var, &ch_copy).unwrap();
                let value = group.value_sv(&cond.state_var);
                cond.state_var.fluent = fluent.clone();
                cond.state_var.args = params;

                match bool::try_from(cond.value) {
                    Ok(true) => {
                        // transform   [s,t] (loc r l) == true  into [s,t] loc r == l
                        cond.value = value.into();
                    }
                    Ok(false) => {
                        // transform   [s,t] (loc r l) == false  into
                        //    [s,t] loc r == ?x    and      ?x != l
                        let var_type = return_type(&cond.state_var.fluent);
                        let var = pb.context.model.new_optional_sym_var(
                            var_type,
                            ch.presence,
                            container_label.var(VarType::Reification),
                        );
                        created_variables.push(var.into());
                        cond.value = var.into();
                        ch.constraints.push(Constraint::neq(var, value));
                    }
                    Err(_) => unreachable!("State variable wrongly identified as substitutable"),
                }
            }
        }

        let mut i = 0;

        // loop to:
        //  - remove all negative effects and store their transition start time
        //  - identify the index of the positive effects
        let mut neg_eff_transition_starts: HashMap<Vec<SAtom>, Time> = HashMap::default();
        let mut pos_effect_indices = Vec::new();
        while i < ch.effects.len() {
            let eff = &mut ch.effects[i];
            if must_substitute(&eff.state_var.fluent) {
                if is_false_assignment(&eff.operation) {
                    // remove effects of the kind  `(at r l) := false`
                    let new_sv_args = group.affected(&eff.state_var, &ch_copy).unwrap();
                    assert!(
                        !neg_eff_transition_starts.contains_key(&new_sv_args),
                        "more than one negative effect"
                    );
                    neg_eff_transition_starts.insert(new_sv_args, eff.transition_start);
                    // remove effect. The counter `i` is not incremented as the next effect to handle is now at `i`
                    ch.effects.remove(i);
                } else {
                    if is_true_assignment(&eff.operation) {
                        pos_effect_indices.push(i);
                    }
                    i += 1;
                }
            } else {
                i += 1;
            }
        }

        for idx in pos_effect_indices {
            let eff = &mut ch.effects[idx];
            // transform `(at r l) := true` into  `(at r) := l`
            let params = group.affected(&eff.state_var, &ch_copy).unwrap();
            let value = group.value_sv(&eff.state_var);

            eff.operation = EffectOp::Assign(value.into());
            if let Some(ts) = neg_eff_transition_starts.get(&params) {
                eff.transition_start = *ts;
            }
            eff.state_var.args = params;
            eff.state_var.fluent = fluent.clone();
        }

        created_variables
    };

    for (id, instance) in pb.chronicles.iter_mut().enumerate() {
        let _created_vars = transform_chronicle(&mut instance.chronicle, Container::Instance(id));
        // no need to add the newly created variables to the parameters as it is not a template
        // and they need not be substituted
    }
    for (id, template) in pb.templates.iter_mut().enumerate() {
        let created_vars = transform_chronicle(&mut template.chronicle, Container::Template(id));
        // add new variables to the chronicle parameters, so they can be substituted upon instantiation of the template
        template.parameters.extend_from_slice(&created_vars);
    }
    // std::process::exit(0)
}

struct CstEff {
    fluent: Arc<Fluent>,
    args: Vec<SymId>,
    value: bool,
}

fn as_cst_eff(eff: &Effect) -> Option<CstEff> {
    let mut c = CstEff {
        fluent: eff.state_var.fluent.clone(),
        args: vec![],
        value: false,
    };
    for x in &eff.state_var.args {
        c.args.push(SymId::try_from(*x).ok()?)
    }
    if let EffectOp::Assign(value) = eff.operation {
        c.value = bool::try_from(value).ok()?;
        Some(c)
    } else {
        None
    }
}
