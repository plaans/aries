#![allow(clippy::needless_range_loop)]

use aries::core::{IntCst, Lit, INT_CST_MAX};
use aries::model::extensions::AssignmentExt;
use aries::model::lang::linear::LinearSum;
use aries::model::lang::IVar;
use aries::solver::search::combinators::WithGeomRestart;
use aries::solver::search::conflicts::ConflictBasedBrancher;
use aries::solver::search::lexical::Lexical;
use aries::solver::search::Brancher;
use env_param::EnvParam;
use itertools::Itertools;
use std::cmp::max;
use std::collections::HashMap;
use std::env;
use std::fmt::{Display, Formatter};
use std::time::Instant;

/// If true, then the objects will be renamed to match the order in which they are treated by the solver
/// from the least interesting to the most. THis is meant to ease debugging.
static RENAME: EnvParam<bool> = EnvParam::new("ARIES_KNAPSACK_RENAME", "false");

#[derive(Debug, Clone)]
pub struct Item {
    pub name: String,
    pub weight: IntCst,
    pub value: IntCst,
}

#[derive(Debug, Clone)]
pub struct Pb {
    pub capacity: IntCst,
    pub optimum: Option<IntCst>,
    pub items: Vec<Item>,
    /// How many time an object may be selected
    pub max_instances: IntCst,
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
            max_instances: 1,
        }
    }

    pub fn is_valid(&self, solution: &Sol) -> bool {
        self.capacity >= solution.weight() && self.optimum.iter().all(|&optimum| optimum == solution.value())
    }

    /// Rename object so that the first (o1) is the one with least value per weight unit
    /// and this value increase afterwards.
    pub fn rename_ordered(&mut self) {
        self.items
            .sort_by_key(|i| num_rational::Rational32::new(i.value, i.weight));
        for (i, item) in self.items.iter_mut().enumerate() {
            item.name = format!("o{}", i + 1);
        }
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

/// Solution as the set of selected items.
#[derive(Debug)]
pub struct Sol {
    pub items: Vec<Item>,
}

impl Sol {
    pub fn empty() -> Self {
        Self { items: vec![] }
    }

    pub fn with(mut self, item: Item) -> Self {
        self.items.push(item);
        self
    }

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

#[derive(Copy, Clone)]
enum SolveMode {
    /// The simple knapsack model with two linear constraints
    Simple,
    /// A constraint encoding and search strategy to favor memoization of the results of part of the tree
    Memoization,
}

fn solve(pb: &Pb, mode: SolveMode) -> Sol {
    let greedy_sol = solve_greedy(pb);
    println!(
        "Greedy solution value: {} (weight: {})",
        greedy_sol.value(),
        greedy_sol.weight()
    );

    let mut model = Model::new();
    let items: Vec<_> = pb
        .items
        .iter()
        .clone()
        // put the least interesting first, to maximize the utility of the learnt clauses
        .sorted_by_key(|i| num_rational::Rational32::new(i.value, i.weight))
        // .rev()
        .collect();
    let max_value = items.iter().map(|i| i.value).sum();

    let vars: Vec<IVar> = items
        .iter()
        .map(|item| model.new_ivar(0, pb.max_instances, &item.name))
        .collect();

    let decisions: Vec<Lit> = vars.iter().map(|v| v.geq(1)).collect();
    let (total_value, brancher): (IVar, Brancher<_>) = match mode {
        SolveMode::Simple => {
            let objective = model.new_ivar(0, INT_CST_MAX, "objective");
            let mut total_weight = LinearSum::zero();
            let mut total_value = LinearSum::zero();
            for i in 0..vars.len() {
                total_weight += vars[i] * items[i].weight;
                total_value += vars[i] * (items[i].value);
            }

            model.enforce(total_weight.leq(pb.capacity), []);
            model.enforce(total_value.clone().leq(objective), []);
            model.enforce(total_value.geq(objective), []);
            let brancher = Box::new(ConflictBasedBrancher::new(decisions));
            let brancher = Box::new(WithGeomRestart::new(100, 1.2, brancher));
            (objective, brancher)
        }
        SolveMode::Memoization => {
            // structure aimed at mimicking Dynamic Programming approaches for the Knapsack
            // Create variables sum of the weights and values from one element until the end of the list
            // this facilitates memoization (like in DP) as it allows representing the fact that
            //   capacity_left_from_i < N => value_from_i < M

            let folder = |(weight_before, value_before): (IVar, IVar), i: usize| {
                let item = &items[i];
                let next_weight = model.new_ivar(0, pb.capacity, format!("weights_from_{}", &item.name));
                let sum_weight = LinearSum::zero() + weight_before + vars[i] * item.weight;
                model.enforce(sum_weight.clone().leq(next_weight), []);
                model.enforce(sum_weight.geq(next_weight), []);

                let next_value = model.new_ivar(0, max_value, format!("value_from_{}", &item.name));
                let sum_value = LinearSum::zero() + value_before + vars[i] * item.value;
                model.enforce(sum_value.clone().leq(next_value), []);
                model.enforce(sum_value.geq(next_value), []);
                (next_weight, next_value)
            };

            // fold from right
            // weight_from_i = i*weight + weight_from_(i+1)
            // value_from_i = i*value + value_from_(i+1)
            let (_total_weight, total_value) = (0..vars.len()).rfold((IVar::ZERO, IVar::ZERO), folder);

            // brancher use lexical search with assignement to max
            // the effect is that we will first pick uninteresting objects (they appear first in the variables)
            // the clauses learnt with be of the form  !o1 v ! o3 v capacity_left_from_i > N v value_from_i < M
            // ideally we would like to get rid of references to o1 and o3 in the clause (attempted in another experimental branch)
            // However this scheme seems to work extremely well. My intuition is that since o1 and o3 (among the first objects) are very unlikely to be picked
            // the clause will become stronger as search progresses (as we will infer clauses forbidding them)
            (total_value, Box::new(Lexical::with_max()))
        }
    };

    let mut solver = Solver::new(model);
    solver.set_brancher_boxed(brancher);

    if let Some(sol) = solver.maximize(total_value).unwrap() {
        let model = solver.model.clone().with_domains(sol.1.as_ref().clone());
        let items: Vec<Item> = vars
            .iter()
            .zip(items.iter())
            // .filter(|(&prez, _)| model.var_domain(prez).lb >= 1)
            .flat_map(|(prez, item)| {
                std::iter::repeat(*item)
                    .take(model.var_domain(*prez).lb as usize)
                    .cloned()
            })
            .collect();
        let solution = Sol { items };
        assert!(pb.is_valid(&solution));
        solver.print_stats();
        solution
    } else {
        panic!("NO SOLUTION");
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();
    println!("Demo knapsack solver.\n> ./knapsack {{num-items}} {{max-instances}} {{mode}}");

    let input = args[1].as_str();
    let n: usize = input.parse().expect("Invalid number as arg");
    let max_instances = if args.len() >= 3 {
        args[2]
            .as_str()
            .parse()
            .expect("Invalid number as max usage of an object")
    } else {
        1
    };
    let mode = if args.len() >= 4 {
        match args[3].as_str() {
            "simple" => SolveMode::Simple,
            "memo" | "memoization" => SolveMode::Memoization,
            invalid => panic!("Invalid mode: {invalid}"),
        }
    } else {
        SolveMode::Memoization
    };

    let pb = gen_problem(n, max_instances);

    if max_instances == 1 {
        solve_dynamic_programming(&pb);
    }
    let pb = if RENAME.get() {
        // rename the objects to ease debugging
        let mut pb = pb.clone();
        pb.rename_ordered();
        pb
    } else {
        pb
    };

    println!("{pb}");
    let solution = solve(&pb, mode);
    println!("{solution}")
}

/// Generates a problem with `n` items.
fn gen_problem(n: usize, max_instances: IntCst) -> Pb {
    let weights = [
        395, 658, 113, 185, 336, 494, 294, 295, 256, 530, 311, 321, 602, 855, 209, 647, 520, 387, 743, 26, 54, 420,
        667, 971, 171, 354, 962, 454, 589, 131, 342, 449, 648, 14, 201, 150, 602, 831, 941, 747, 444, 982, 732, 350,
        683, 279, 667, 400, 441, 786, 309, 887, 189, 119, 209, 532, 461, 420, 14, 788, 691, 510, 961, 528, 538, 476,
        49, 404, 761, 435, 729, 245, 204, 401, 347, 674, 75, 40, 882, 520, 692, 104, 512, 97, 713, 779, 224, 357, 193,
        431, 442, 816, 920, 28, 143, 388, 23, 374, 905, 942,
    ];
    let values = [
        71, 15, 100, 37, 77, 28, 71, 30, 40, 22, 28, 39, 43, 61, 57, 100, 28, 47, 32, 66, 79, 70, 86, 86, 22, 57, 29,
        38, 83, 73, 91, 54, 61, 63, 45, 30, 51, 5, 83, 18, 72, 89, 27, 66, 43, 64, 22, 23, 22, 72, 10, 29, 59, 45, 65,
        38, 22, 68, 23, 13, 45, 34, 63, 34, 38, 30, 82, 33, 64, 100, 26, 50, 66, 40, 85, 71, 54, 25, 100, 74, 96, 62,
        58, 21, 35, 36, 91, 7, 19, 32, 77, 70, 23, 43, 78, 98, 30, 12, 76, 38,
    ];
    let capacity = 2000;

    let items = (0..n)
        .map(|i| {
            let i = i % weights.len();
            Item {
                name: format!("o{i}"),
                weight: weights[i],
                value: values[i],
            }
        })
        .collect();
    Pb {
        capacity: (capacity * n / weights.len()) as IntCst * max_instances,
        optimum: if n == weights.len() && max_instances == 1 {
            Some(1161)
        } else {
            None
        },
        items,
        max_instances,
    }
}

fn solve_greedy(pb: &Pb) -> Sol {
    let init = Sol::empty();
    let items = pb
        .items
        .iter()
        .sorted_by_key(|i| num_rational::Rational32::new(i.weight, i.value))
        .collect_vec();
    items.iter().fold(init, |mut sol, item| {
        for _ in 0..pb.max_instances {
            if sol.weight() + item.weight <= pb.capacity {
                sol = sol.with((*item).clone())
            }
        }
        sol
    })
}

fn solve_dynamic_programming(pb: &Pb) {
    let start = Instant::now();
    let items = &pb.items;
    let mut memo = HashMap::new();

    let opti = m(items.len() - 1, pb.capacity, items, &mut memo);
    let dur = start.elapsed();

    println!(
        "Dynamic programming solution value: {opti}  (in {dur:?})  [#entries: {}]",
        memo.len()
    );
}

fn m(i: usize, j: IntCst, items: &[Item], value: &mut HashMap<(usize, IntCst), IntCst>) -> IntCst {
    if i == 0 || j <= 0 {
        value.insert((i, j), 0);
        return 0;
    }
    if !value.contains_key(&(i - 1, j)) {
        // force its computation
        m(i - 1, j, items, value);
    }
    let w = items[i].weight;
    let val = items[i].value;
    let val_ij = if w > j {
        // cannot be added
        value[&(i - 1, j)]
    } else {
        if !value.contains_key(&(i - 1, j - w)) {
            m(i - 1, j - w, items, value);
        }
        max(value[&(i - 1, j)], val + value[&(i - 1, j - w)])
    };
    value.insert((i, j), val_ij);
    val_ij
}

#[cfg(test)]
mod tests {
    use crate::{gen_problem, solve, Pb, SolveMode};

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

            assert!(pb.is_valid(&solve(&pb, SolveMode::Simple)));
            assert!(pb.is_valid(&solve(&pb, SolveMode::Memoization)));
        }

        for max_instances in 1..=1 {
            // the simple algorithm for the 0-1 Knapsack
            for i in 0..30 {
                let pb = gen_problem(i, max_instances);
                let simpl_sol = solve(&pb, SolveMode::Simple);
                let memo_sol = solve(&pb, SolveMode::Memoization);
                assert_eq!(simpl_sol.value(), memo_sol.value())
            }
        }
    }
}
