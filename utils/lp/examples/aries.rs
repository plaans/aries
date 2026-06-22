use std::collections::HashMap;

use minilp::{ComparisonOp, OptimizationDirection, Problem, Variable};

type IntCst = i32;

#[derive(Debug)]
struct Lit {
    x: Variable,
    op: ComparisonOp,
    bound: IntCst,
}

#[derive(Debug)]
struct Wrapper {
    problem: Problem,
    lit_vec: Vec<Lit>,
    memory_s: HashMap<Vec<(IntCst, IntCst)>, Variable>,
    memory_x: HashMap<IntCst, Variable>,
}

impl Wrapper {
    fn new() -> Self {
        Self {
            problem: Problem::new(OptimizationDirection::Minimize), // The direction doesn't matter as we want to test the satisfiability
            lit_vec: Vec::new(),
            memory_s: HashMap::new(),
            memory_x: HashMap::new(),
        }
    }

    fn get_opposite_linear_sum(linear_sum: &[(IntCst, IntCst)]) -> Vec<(IntCst, IntCst)> {
        let mut opp = Vec::new();
        for (x, coeff) in linear_sum {
            opp.push((*x, -coeff));
        }
        opp
    }

    fn add_x_var(&mut self, x: IntCst) {
        let var = self.problem.add_var(0.0, (f64::NEG_INFINITY, f64::INFINITY));

        self.memory_x.insert(x, var);
    }

    fn add_s_var(&mut self, linear_sum: &[(IntCst, IntCst)]) -> Variable {
        for (x, _) in linear_sum {
            if !self.memory_x.contains_key(x) {
                self.add_x_var(*x);
            }
        }

        // If we depend on only one x variable with a factor 1, no need to create a s var
        if linear_sum.len() == 1 && linear_sum[0].1 == 1 {
            return *self.memory_x.get(&linear_sum[0].0).unwrap();
        }

        let s = self.problem.add_var(0.0, (f64::NEG_INFINITY, f64::INFINITY));

        let mut expr = vec![(s, -1.0)];

        for (x, coeff) in linear_sum {
            let var = *self.memory_x.get(x).unwrap();
            expr.push((var, *coeff as f64));
        }

        // We force s to be egal to our linear sum
        self.problem.add_constraint(expr, ComparisonOp::Eq, 0.0);

        self.memory_s.insert(linear_sum.to_vec(), s);

        s
    }

    fn process_constraint(&mut self, linear_sum: &[(IntCst, IntCst)], bound: IntCst) {
        let opp_lin_sum = Wrapper::get_opposite_linear_sum(linear_sum);
        let lit: Lit;
        if self.memory_s.contains_key(linear_sum) {
            let &s = self.memory_s.get(linear_sum).unwrap();
            lit = Lit {
                x: s,
                op: ComparisonOp::Le,
                bound,
            };
        } else if self.memory_s.contains_key(&opp_lin_sum) {
            let &s = self.memory_s.get(&opp_lin_sum).unwrap();
            lit = Lit {
                x: s,
                op: ComparisonOp::Ge,
                bound: -bound,
            };
        } else {
            let s = self.add_s_var(linear_sum);

            lit = Lit {
                x: s,
                op: ComparisonOp::Le,
                bound,
            };
        }

        self.lit_vec.push(lit);
    }

    fn solve_iterative(&self) {
        let solve_res = self.problem.solve();

        debug_assert!(solve_res.is_ok(), "Error while solving with no active constraints");

        let mut solution = solve_res.unwrap();

        for lit in &self.lit_vec {
            let result = solution.add_constraint([(lit.x, 1.0)], lit.op, lit.bound as f64);

            if result.is_err() {
                println!("Unsatisifable constraints detected after adding {:?}", lit);
                break;
            }

            solution = result.unwrap();
        }
    }
}

fn main() {
    let mut my_wrapper = Wrapper::new();

    my_wrapper.process_constraint(&[(0, 1), (1, 1)], 2);
    my_wrapper.process_constraint(&[(0, -1), (1, -1)], -4);
    my_wrapper.process_constraint(&[(3, 1)], 7);

    let opp_lin_sum = Wrapper::get_opposite_linear_sum(&[(0, 1), (1, -5), (2, 9), (5, 17)]);
    println!("Opposite linear sum: {:?}", opp_lin_sum);

    my_wrapper.solve_iterative();

    println!("{:?}", my_wrapper);
}
