use crate::chronicles::analysis::is_static;
use crate::chronicles::{EffectOp, Fluent, Problem};
use aries::core::Lit;
use aries::model::lang::{Atom, SAtom};
use aries::model::symbols::{SymId, TypedSym};
use aries::model::types::TypeId;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};
use std::fmt::Write;

#[derive(Eq, PartialEq, Copy, Clone, Hash, Debug)]
enum Value {
    SymType(TypeId),
    SymCst(TypedSym),
    BoolType,
    BoolCst(bool),
}

impl Value {
    #[allow(unused)]
    pub fn is_constant(&self) -> bool {
        match self {
            Value::SymCst(_) | Value::BoolCst(_) => true,
            Value::SymType(_) | Value::BoolType => false,
        }
    }
    fn write(&self, out: &mut String, pb: &Problem) {
        match self {
            Value::SymType(t) => {
                let tpe = pb.context.model.shape.symbols.types.from_id(*t);
                write!(out, "{tpe}").unwrap()
            }
            Value::SymCst(ts) => {
                let sym = pb.context.model.shape.symbols.symbol(ts.sym);
                write!(out, "{sym}").unwrap()
            }
            Value::BoolType => write!(out, "?").unwrap(),
            Value::BoolCst(b) => write!(out, "{b}").unwrap(),
        }
    }
}

#[derive(Eq, PartialEq, Clone, Hash, Debug)]
struct GAtom {
    fluent: SymId,
    params: Vec<Value>,
    value: Value,
}

impl GAtom {
    pub fn format(&self, pb: &Problem) -> String {
        let mut out = String::with_capacity(64);
        let fluent = pb.context.model.shape.symbols.symbol(self.fluent);
        write!(out, "{fluent}(").unwrap();
        for (i, arg) in self.params.iter().enumerate() {
            if i > 0 {
                write!(out, ", ").unwrap()
            }
            arg.write(&mut out, pb)
        }
        write!(out, "):").unwrap();
        self.value.write(&mut out, pb);

        out
    }
}

fn atom_to_gatom(a: impl Into<Atom>) -> Value {
    let a = a.into();
    match a {
        Atom::Bool(b) => {
            if b == Lit::TRUE {
                Value::BoolCst(true)
            } else if b == Lit::FALSE {
                Value::BoolCst(false)
            } else {
                Value::BoolType
            }
        }
        Atom::Int(_) => todo!(),
        Atom::Fixed(_) => todo!(),
        Atom::Sym(SAtom::Cst(tsym)) => Value::SymCst(tsym),
        Atom::Sym(SAtom::Var(s)) => Value::SymType(s.tpe),
    }
}

fn value_of(fluent: &Fluent, params: &[SAtom], value: Atom) -> GAtom {
    let fluent = fluent.sym;
    let params = params.iter().map(|sa| atom_to_gatom(*sa)).collect_vec();
    let value = atom_to_gatom(value);

    GAtom { fluent, params, value }
}

pub fn find_useless_supports(pb: &Problem) -> HashSet<CausalSupport> {
    let mut useful_values = HashSet::new(); // TODO: extend with goals

    for ch in &pb.templates {
        for cond in &ch.chronicle.conditions {
            let gval = value_of(&cond.state_var.fluent, &cond.state_var.args, cond.value);
            useful_values.insert(gval);
        }
    }

    println!("Useful values:");
    for v in &useful_values {
        println!(" - {}", v.format(pb));
    }

    println!("Continuous fluents: ");
    let mut detrimental_conditions = HashSet::new();
    for f in &pb.context.fluents {
        if is_static(f.as_ref(), pb) {
            continue;
        }
        gather_detrimental_supports(f.as_ref(), pb, &useful_values, &mut detrimental_conditions)
    }
    detrimental_conditions
}

#[allow(unused)]
/// Function to build an operator graph
fn build_graph(pb: &Problem) {
    let mut useful_values = HashSet::new(); // TODO: extend with goals
    let g = &mut String::new();
    writeln!(g, "digraph ops {{").unwrap();
    for (i, ch) in pb.templates.iter().enumerate() {
        println!("{:?}", ch.label);
        writeln!(g, "  {i} [shape=\"rectangle\", label=\"{}\"];", &ch.label).unwrap();
        println!("  cond:");
        for cond in &ch.chronicle.conditions {
            let gval = value_of(&cond.state_var.fluent, &cond.state_var.args, cond.value);
            println!("  - {}", gval.format(pb));
            writeln!(g, "  \"{}\" -> {i};", gval.format(pb)).unwrap();
            useful_values.insert(gval);
        }
        println!("  effs:");
        for eff in &ch.chronicle.effects {
            let EffectOp::Assign(value) = eff.operation else {
                continue;
            };
            let gval = value_of(&eff.state_var.fluent, &eff.state_var.args, value);
            println!("  - {}", gval.format(pb));
            writeln!(g, "  {i} -> \"{}\";", gval.format(pb)).unwrap();
        }
    }
    write!(g, "}}");
    std::fs::write("/tmp/graph.dot", g).expect("Unable to write file");
    // std::process::exit(0)

    println!("Useful values:");
    for v in &useful_values {
        println!(" - {}", v.format(pb));
    }

    println!("Continuous fluents: ");
    let mut detrimental_conditions = HashSet::new();
    for f in &pb.context.fluents {
        if is_static(f.as_ref(), pb) {
            continue;
        }
        gather_detrimental_supports(f.as_ref(), pb, &useful_values, &mut detrimental_conditions)
    }
}

#[derive(Hash, Copy, Clone, Eq, PartialEq, Debug)]
pub struct TemplateCondID {
    /// id of the chronicle template in which the condition occurs
    pub template_id: usize,
    /// Index of the condition in the template's conditions
    pub cond_id: usize,
}
#[derive(Hash, Copy, Clone, Eq, PartialEq, Debug)]
pub struct TemplateEffID {
    /// id of the chronicle template in which the condition occurs
    pub template_id: usize,
    /// Index of the effect in the template's effects
    pub effect_id: usize,
}

/// Represents a potential causal support from the effect of a template to the condition of a template
#[derive(Hash, Copy, Clone, Eq, PartialEq, Debug)]
pub struct CausalSupport {
    supporter: TemplateEffID,
    condition: TemplateCondID,
}

impl CausalSupport {
    pub fn new(eff_template: usize, eff_id: usize, cond_template: usize, cond_id: usize) -> Self {
        Self {
            supporter: TemplateEffID {
                template_id: eff_template,
                effect_id: eff_id,
            },
            condition: TemplateCondID {
                template_id: cond_template,
                cond_id: cond_id,
            },
        }
    }
    fn transitive(eff: &Transition, cond: &Transition) -> Self {
        Self {
            supporter: TemplateEffID {
                template_id: eff.template_id,
                effect_id: eff.eff_id,
            },
            condition: TemplateCondID {
                template_id: cond.template_id,
                cond_id: cond.cond_id,
            },
        }
    }
}

struct Transition {
    template_id: usize,
    cond_id: usize,
    eff_id: usize,
    pre: GAtom,
    post: GAtom,
}

struct ConditionTemplate {
    template_id: usize,
    cond_id: usize,
    pre: GAtom,
}

fn find_conditions(fluent: &Fluent, pb: &Problem) -> Vec<ConditionTemplate> {
    let mut conditions = Vec::new();
    for (template_id, ch) in pb.templates.iter().enumerate() {
        let ch = &ch.chronicle;
        for (cond_id, c) in ch.conditions.iter().enumerate() {
            if c.state_var.fluent.as_ref() != fluent {
                continue;
            }
            let pre = value_of(&c.state_var.fluent, &c.state_var.args, c.value);
            conditions.push(ConditionTemplate {
                template_id,
                cond_id,
                pre,
            })
        }
    }
    conditions
}

fn find_transitions(fluent: &Fluent, pb: &Problem) -> Vec<Transition> {
    let mut transitions = Vec::new();
    for (template_id, ch) in pb.templates.iter().enumerate() {
        let ch = &ch.chronicle;
        for (eff_id, e) in ch.effects.iter().enumerate() {
            if e.state_var.fluent.as_ref() != fluent {
                continue;
            }
            for (cond_id, c) in ch.conditions.iter().enumerate() {
                let pre = value_of(&c.state_var.fluent, &c.state_var.args, c.value);

                if c.state_var == e.state_var && c.end == e.transition_start {
                    let EffectOp::Assign(val) = e.operation else { panic!() };
                    let post = value_of(&e.state_var.fluent, &e.state_var.args, val);
                    transitions.push(Transition {
                        template_id,
                        cond_id,
                        eff_id,
                        pre,
                        post,
                    });
                    break;
                }
            }
            debug_assert!(
                {
                    let t = transitions.last().unwrap();
                    t.template_id == template_id && t.eff_id == eff_id
                },
                "THe effect did not receive any matching condition (thus it is not a transition)"
            );
        }
    }
    transitions
}

fn gather_detrimental_supports(
    fluent: &Fluent,
    pb: &Problem,
    useful_values: &HashSet<GAtom>,
    detrimentals: &mut HashSet<CausalSupport>,
) {
    let mut external_contributors = HashSet::new();
    for ch in &pb.templates {
        let mut conds = HashMap::new();
        for c in &ch.chronicle.conditions {
            if c.state_var.fluent.as_ref() == fluent {
                conds.insert(
                    (&c.state_var, c.end),
                    value_of(&c.state_var.fluent, &c.state_var.args, c.value),
                );
            }
        }
        for e in &ch.chronicle.effects {
            if e.state_var.fluent.as_ref() == fluent {
                let key = (&e.state_var, e.transition_start);
                if !conds.contains_key(&key) {
                    return; // the fluent is not continuous
                } else {
                    // conds.remove(&key).unwrap();
                }
            } else {
                let EffectOp::Assign(value) = e.operation else { panic!() };
                let atom = value_of(&e.state_var.fluent, &e.state_var.args, value);
                if useful_values.contains(&atom) {
                    external_contributors.extend(conds.values().cloned());
                }
            }
        }
    }
    let single_useful_value = if external_contributors.len() != 1 {
        None
    } else {
        let v = external_contributors.iter().next().unwrap();
        match v.value {
            Value::SymCst(_) | Value::BoolCst(_) => Some(v.clone()),
            _ => None, // may have different instantiations
        }
    };

    println!(" - {}", &fluent.name);
    for c in &external_contributors {
        println!("   - {}", c.format(pb));
    }

    // gather all values that are affected by non-optional chronicles (ie, non-templates)
    let mut initial_values = HashSet::new();
    for ch in &pb.chronicles {
        for e in &ch.chronicle.effects {
            if e.state_var.fluent.as_ref() == fluent {
                let EffectOp::Assign(value) = e.operation else { panic!() };
                let atom = value_of(&e.state_var.fluent, &e.state_var.args, value);
                initial_values.insert(atom.value);
            }
        }
    }

    let conditions = find_conditions(fluent, pb);
    let transitions = find_transitions(fluent, pb);
    let supporters = |val: &GAtom| transitions.iter().filter(move |t| &t.post == val).collect_vec();
    let supported = |val: &GAtom| transitions.iter().filter(move |t| &t.pre == val).collect_vec();

    if let Some(useful) = single_useful_value {
        // we have a single useful value, any support for a transition that moves away is detrimental
        // a transition that establishes the useful value must not be used only to enable a transition away from it
        println!("single useful: {}", useful.format(pb));

        for eff in &transitions {
            if eff.pre == useful {
                for cond in supported(&eff.post) {
                    detrimentals.insert(CausalSupport::transitive(eff, cond));
                }
            }
        }

        if transitions.iter().all(|t| t.pre == useful && t.post == useful) {
            for c in conditions.iter().filter(|c| c.pre == useful) {
                for eff in supporters(&c.pre) {
                    detrimentals.insert(CausalSupport {
                        supporter: TemplateEffID {
                            template_id: eff.template_id,
                            effect_id: eff.eff_id,
                        },
                        condition: TemplateCondID {
                            template_id: c.template_id,
                            cond_id: c.cond_id,
                        },
                    });
                }
            }
        }
    } /* Deactivated as it may be incorrect in some lifted cases where unless checking that the return (supported)
           transition ends up at the same value
      else {
          // detect pattern where the is a single transition value:
          // - always is the initial value
          // - is not useful in itself
          // - is the source/target of all transition to/from useful values
          let from_useful = transitions
              .iter()
              .filter(|t| external_contributors.contains(&t.pre))
              .collect_vec();
          let post_useful: HashSet<_> = from_useful.iter().map(|t| &t.post).collect();
          let to_useful = transitions
              .iter()
              .filter(|t| external_contributors.contains(&t.post))
              .collect_vec();
          let pre_useful: HashSet<_> = to_useful.iter().map(|t| &t.pre).collect();
          if post_useful.len() == 1 && post_useful == pre_useful {
              let transition_value = post_useful.iter().next().copied().unwrap();
              // true if there is a unique tranisition value (ie, it does not correspond to a type that could have ultiple values)
              let transition_value_is_unique = transition_value.value.is_constant();
              // true if the state variables always start from the initial value
              let transition_is_initial = initial_values.iter().all(|a| a == &transition_value.value);

              if !external_contributors.contains(&transition_value) && transition_value_is_unique && transition_is_initial
              {
                  // we have our single transition value,
                  // mark as detrimental all transitions to it.
                  // proof: any transition to it must be preceded by transition from it (with no other side effects)
                  for t1 in supporters(transition_value) {
                      for t2 in supported(transition_value) {
                          if t1.post == t2.pre {
                              // this is a transition from   `transition_value -> useful_value -> transition_value`
                              detrimentals.insert(CausalSupport::transitive(t1, t2));
                          }
                      }
                  }
              }

          }
      } */
}
