use aries_core::IntCst;
use aries_cp::*;
use aries_model::lang::linear::LinearSum;
use aries_model::lang::IVar;

#[derive(Debug)]
struct Item {
    name: String,
    weight: IntCst,
    value: IntCst,
}

impl Item {
    pub fn new(name: impl Into<String>, weight: IntCst, value: IntCst) -> Item {
        Item {
            name: name.into(),
            weight,
            value,
        }
    }
}

#[derive(Debug)]
struct Pb {
    capacity: IntCst,
    items: Vec<Item>,
}

type Var = String;

type Model = aries_model::Model<Var>;
type Solver = aries_solver::solver::Solver<Var>;

// #[derive(Debug, StructOpt)]
// #[structopt(name = "knapsack")]
// struct Opt {
//     /// File containing the jobshop instance to solve.
//     file: String,
//     /// When set, the solver will fail if the found solution does not have this makespan.
//     #[structopt(long = "expected-value")]
//     expected_value: Option<u32>,
// }

fn solve(pb: &Pb) -> IntCst {
    let mut model = Model::new();

    let vars: Vec<IVar> = pb.items.iter().map(|item| model.new_ivar(0, 1, &item.name)).collect();

    let neg_value = model.new_ivar(-1000, 0, "objective");

    let mut total_weight = LinearSum::zero();
    let mut total_value = LinearSum::zero() + neg_value;
    for i in 0..vars.len() {
        total_weight += vars[i] * pb.items[i].weight;
        total_value += vars[i] * (pb.items[i].value);
    }

    // model.enforce(total_weight.clone().geq(1));
    model.enforce(total_weight.leq(pb.capacity));
    model.enforce(total_value.clone().leq(0));
    model.enforce(total_value.geq(0));

    let mut solver = Solver::new(model);
    solver.add_theory(Cp::new);
    if let Some(sol) = solver.minimize(neg_value).unwrap() {
        println!("SOLUTION");
        let model = solver.model.clone().with_domains(sol.1.as_ref().clone());
        dbg!(sol.0);
        model.print_state();
        -sol.0
    } else {
        panic!("NO SOLUTION");
    }
}

fn main() {
    let pb = Pb {
        capacity: 10,
        items: vec![Item::new("a", 1, 1), Item::new("b", 6, 5), Item::new("c", 6, 3)],
    };

    println!("{:?}", pb);
    solve(&pb);
}
