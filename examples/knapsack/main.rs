#![allow(clippy::needless_range_loop)]

use aries::core::{IntCst, Lit, INT_CST_MAX};
use aries::model::extensions::AssignmentExt;
use aries::model::lang::linear::LinearSum;
use aries::model::lang::IVar;
use std::env;
use std::fmt::{Display, Formatter};

#[derive(Debug, Clone)]
struct Item {
    name: String,
    weight: IntCst,
    value: IntCst,
}

#[derive(Debug)]
struct Pb {
    capacity: IntCst,
    optimum: Option<IntCst>,
    items: Vec<Item>,
}

impl Pb {
    pub fn parse(input: &str) -> Pb {
        let mut capacity: Option<IntCst> = None;
        let mut optimum = None;
        let mut items = Vec::with_capacity(8);

        for line in input.split(';') {
            let tokens: Vec<_> = line.split_whitespace().collect();
            match &tokens.as_slice() {
                ["cap", cap] => {
                    assert!(capacity.is_none());
                    capacity = Some(cap.parse().unwrap())
                }
                ["opt", opt] => {
                    assert!(optimum.is_none());
                    optimum = Some(opt.parse().unwrap())
                }
                [name, value, weight] => items.push(Item {
                    name: name.to_string(),
                    weight: weight.parse().unwrap(),
                    value: value.parse().unwrap(),
                }),
                _ => panic!(),
            }
        }

        Pb {
            capacity: capacity.unwrap(),
            optimum,
            items,
        }
    }

    pub fn is_valid(&self, solution: &Sol) -> bool {
        self.capacity >= solution.weight() && self.optimum.iter().all(|&optimum| optimum == solution.value())
    }
}

impl Display for Pb {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Problem:")?;
        writeln!(f, "  capacity: {}", self.capacity)?;
        if let Some(optimum) = self.optimum {
            writeln!(f, "  optimum: {optimum}")?;
        }
        writeln!(f, "  Items:")?;
        for item in &self.items {
            writeln!(f, "    {}\tweight: {}\tvalue: {}", &item.name, item.weight, item.value)?;
        }
        Ok(())
    }
}

#[derive(Debug)]
struct Sol {
    items: Vec<Item>,
}

impl Sol {
    pub fn weight(&self) -> IntCst {
        self.items.iter().map(|i| i.weight).sum()
    }
    pub fn value(&self) -> IntCst {
        self.items.iter().map(|i| i.value).sum()
    }
}

impl Display for Sol {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "Solution:")?;
        writeln!(f, "  total value: {}", self.value())?;
        writeln!(f, "  total weight: {}", self.weight())?;
        writeln!(f, "  Items:")?;
        for item in &self.items {
            writeln!(f, "    {}\tweight: {}\tvalue: {}", &item.name, item.weight, item.value)?;
        }
        Ok(())
    }
}

type Var = String;

type Model = aries::model::Model<Var>;
type Solver = aries::solver::Solver<Var>;

fn solve(pb: &Pb) -> Sol {
    let mut model = Model::new();

    let vars: Vec<IVar> = pb.items.iter().map(|item| model.new_ivar(0, 1, &item.name)).collect();

    let objective = model.new_ivar(0, INT_CST_MAX, "objective");

    let mut total_weight = LinearSum::zero();
    let mut total_value = LinearSum::zero();
    for i in 0..vars.len() {
        total_weight += vars[i] * pb.items[i].weight;
        total_value += vars[i] * (pb.items[i].value);
    }

    // model.enforce(total_weight.clone().geq(1));
    model.enforce(total_weight.leq(pb.capacity));
    model.enforce(total_value.clone().leq(objective));
    model.enforce(total_value.geq(objective));

    let mut solver = Solver::new(model);
    if let Some(sol) = solver.maximize(objective).unwrap() {
        let model = solver.model.clone().with_domains(sol.1.as_ref().clone());
        let items: Vec<Item> = vars
            .iter()
            .zip(pb.items.iter())
            .filter(|(&prez, _)| model.var_domain(prez).lb >= 1)
            .map(|(_, item)| item.clone())
            .collect();
        let solution = Sol { items };
        assert!(pb.is_valid(&solution));
        solution
    } else {
        panic!("NO SOLUTION");
    }
}

/// Alternate solver where each value/weight is encoded as an optional variable.
#[allow(unused)]
fn solve_optional(pb: &Pb) -> Sol {
    let mut model = Model::new();

    let presence_vars: Vec<Lit> = pb
        .items
        .iter()
        .map(|item| model.new_presence_variable(Lit::TRUE, &item.name).true_lit())
        .collect();
    let weight_vars: Vec<_> = presence_vars
        .iter()
        .copied()
        .enumerate()
        .map(|(i, prez)| {
            model
                .new_optional_ivar(
                    pb.items[i].weight,
                    pb.items[i].weight,
                    prez,
                    format!("{}_weight", pb.items[i].name),
                )
                .or_zero()
        })
        .collect();
    let value_vars: Vec<_> = presence_vars
        .iter()
        .copied()
        .enumerate()
        .map(|(i, prez)| {
            model
                .new_optional_ivar(
                    pb.items[i].value,
                    pb.items[i].value,
                    prez,
                    format!("{}_value", pb.items[i].name),
                )
                .or_zero()
        })
        .collect();

    let objective = model.new_ivar(0, 1000, "objective");

    let total_weight = LinearSum::of(weight_vars);
    let total_value = LinearSum::of(value_vars);

    // model.enforce(total_weight.clone().geq(1));
    model.enforce(total_weight.leq(pb.capacity));
    model.enforce(total_value.clone().leq(objective));
    model.enforce(total_value.geq(objective));

    let mut solver = Solver::new(model);
    if let Some(sol) = solver.maximize(objective).unwrap() {
        let model = solver.model.clone().with_domains(sol.1.as_ref().clone());
        model.print_state();
        let items: Vec<Item> = presence_vars
            .iter()
            .zip(pb.items.iter())
            .filter(|(&prez, _)| model.entails(prez))
            .map(|(_, item)| item.clone())
            .collect();
        let solution = Sol { items };
        assert_eq!(solution.value(), sol.0);
        assert!(pb.is_valid(&solution));
        solution
    } else {
        panic!("NO SOLUTION");
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    println!("{args:?}");
    let input = args[1].as_str();

    let pb = Pb::parse(input);

    println!("{pb}");
    let solution = solve(&pb);
    println!("{solution}")
}

#[cfg(test)]
mod tests {
    use crate::{solve, solve_optional, Pb};

    static PROBLEMS: &[&str] = &[
        "cap 10 ; opt 6 ; a 1 1 ; b 5 6 ; c 3 6",
        "cap 5 ; opt 5 ; a 1 1 ; b 1 1 ; c 1 1 ; d 1 1 ; e 1 1 ; f 1 1; g 1 1 ; h 1 1 ; i 1 1",
        "cap 0 ; opt 0 ; a 1 1 ; b 1 1 ; c 1 1 ; d 1 1 ; e 1 1 ; f 1 1; g 1 1 ; h 1 1 ; i 1 1",
        "cap 9 ; opt 9 ; a 1 1 ; b 1 1 ; c 1 1 ; d 1 1 ; e 1 1 ; f 1 1; g 1 1 ; h 1 1 ; i 1 1",
        "cap 10 ; opt 9 ; a 1 1 ; b 1 1 ; c 1 1 ; d 1 1 ; e 1 1 ; f 1 1; g 1 1 ; h 1 1 ; i 1 1",
    ];

    #[test]
    fn test_knapsack() {
        for &pb_str in PROBLEMS {
            println!("=======================");
            let pb = Pb::parse(pb_str);
            println!("{pb}");

            assert!(pb.is_valid(&solve(&pb)));
            assert!(pb.is_valid(&solve_optional(&pb)));
        }
    }
}
