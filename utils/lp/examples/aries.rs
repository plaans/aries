use std::collections::HashMap;

use minilp::{Bound, ComparisonOp, Error, OptimizationDirection, Problem, Variable};

type IntCst = i32;

#[derive(Debug, Clone)]
struct Lit {
    var: Variable,
    bound: Bound,
    val: IntCst,
}

#[derive(Debug)]
struct IntBounds {
    lower: IntCst,
    upper: IntCst,
}

// No need to store a bound or an op as all of our constraints are equalities between an s variable and linear sum of x variables
#[derive(Debug)]
struct IntegerConstraint {
    lin_sum: Vec<(Variable, IntCst)>,
}

#[derive(Debug)]
pub struct Interface {
    problem: Problem,

    // Used to store an exact version of our original problem with integers
    bounds: Vec<IntBounds>,
    constraints: Vec<IntegerConstraint>,

    lit_vec: Vec<Lit>,

    memory_s: HashMap<Vec<(usize, IntCst)>, Variable>,
    memory_x: HashMap<usize, Variable>,
    memory_x_reversed: HashMap<Variable, usize>,
}

impl Default for Interface {
    fn default() -> Self {
        Self::new()
    }
}

impl Interface {
    /// Create a new empty interface instance
    pub fn new() -> Self {
        Self {
            problem: Problem::new(OptimizationDirection::Minimize), // The direction doesn't matter as we want to test the satisfiability
            bounds: Vec::new(),
            constraints: Vec::new(),

            lit_vec: Vec::new(),

            memory_s: HashMap::new(),
            memory_x: HashMap::new(),
            memory_x_reversed: HashMap::new(),
        }
    }

    // Create a new solver variable both for Problem and the mirror of our problem
    fn create_variable(&mut self, lb: IntCst, ub: IntCst) -> Variable {
        let var = self.problem.add_var(0.0, (lb as f64, ub as f64));

        debug_assert_eq!(var.idx(), self.bounds.len());

        self.bounds.push(IntBounds { lower: lb, upper: ub });
        var
    }

    // Modify the bound of an existing variable (necessary to verify the certificate)
    fn set_bound(&mut self, var: Variable, bound: Bound, val: IntCst) {
        debug_assert!(var.idx() < self.bounds.len());

        match bound {
            Bound::Lower => self.bounds[var.idx()].lower = val,
            Bound::Upper => self.bounds[var.idx()].upper = val,
        }
    }

    // Create a new solver constraint both for Problem and the mirror of our problem, we assume we only have equality constraint
    fn add_constraint(&mut self, lin_sum: Vec<(Variable, IntCst)>) {
        let float_lin_sum: Vec<(Variable, f64)> = lin_sum.iter().map(|&(var, coef)| (var, coef as f64)).collect();

        self.problem.add_constraint(float_lin_sum, ComparisonOp::Eq, 0.0);

        self.constraints.push(IntegerConstraint { lin_sum });
    }

    // Return the maximum value that the given linear sum can tak respect to its bounds
    fn max_lin_sum(&self, lin_sum: &[i128]) -> i128 {
        lin_sum
            .iter()
            .enumerate()
            .map(|(i, &coeff)| {
                if coeff == 0 {
                    0
                } else if coeff < 0 {
                    self.bounds[i].lower as i128 * coeff
                } else {
                    self.bounds[i].upper as i128 * coeff
                }
            })
            .sum()
    }

    // Return the minimum value that the given linear sum can tak respect to its bounds
    fn min_lin_sum(&self, lin_sum: &[i128]) -> i128 {
        lin_sum
            .iter()
            .enumerate()
            .map(|(i, &coeff)| {
                if coeff == 0 {
                    0
                } else if coeff > 0 {
                    self.bounds[i].lower as i128 * coeff
                } else {
                    self.bounds[i].upper as i128 * coeff
                }
            })
            .sum()
    }

    // Convert a certificate with f64 coefficients in an a equivalent i128 certificate
    fn convert_certificate_i128(cert: &[f64]) -> Vec<i128> {
        let coef = 2.0_f64.powi(52);

        cert.iter().map(|x| (x * coef) as i128).collect()
    }

    /// Verify the certificate of unsatisfiability
    pub fn is_certificate_valid(&self, cert: &[f64]) -> bool {
        debug_assert_eq!(cert.len(), self.constraints.len());

        let mut lin_sum: Vec<i128> = vec![0; self.bounds.len()];

        let cert_i128 = Interface::convert_certificate_i128(cert);

        for (const_i, &coef_cert) in cert_i128.iter().enumerate() {
            if coef_cert == 0 {
                continue;
            }

            for &(var_i, coef_var) in self.constraints[const_i].lin_sum.iter() {
                lin_sum[var_i.idx()] += coef_cert * coef_var as i128;
            }
        }

        let min_lin_sum = self.min_lin_sum(&lin_sum);
        let max_lin_sum = self.max_lin_sum(&lin_sum);

        // println!("Int cert max: {max_lin_sum}, min: {min_lin_sum}");

        max_lin_sum < 0 || min_lin_sum > 0
    }

    // Return a linear sum wich is the opposite in termes of coefficient that the one given
    fn get_opposite_linear_sum(linear_sum: &[(usize, IntCst)]) -> Vec<(usize, IntCst)> {
        let mut opp = Vec::new();
        for (x, coeff) in linear_sum {
            opp.push((*x, -coeff));
        }
        opp
    }

    fn add_x_var(&mut self, x: usize) {
        let var = self.create_variable(IntCst::MIN, IntCst::MAX);

        self.memory_x.insert(x, var);
        self.memory_x_reversed.insert(var, x);
    }

    fn add_s_var(&mut self, linear_sum: &[(usize, IntCst)]) -> Variable {
        for (x, _) in linear_sum {
            if !self.memory_x.contains_key(x) {
                self.add_x_var(*x);
            }
        }

        // If we depend on only one x variable with a factor 1, no need to create a s var
        if linear_sum.len() == 1 && linear_sum[0].1 == 1 {
            return *self.memory_x.get(&linear_sum[0].0).unwrap();
        }

        let s = self.create_variable(IntCst::MIN, IntCst::MAX);

        let mut lin_sum = vec![(s, -1)];

        for (x, coeff) in linear_sum {
            let var = *self.memory_x.get(x).unwrap();
            lin_sum.push((var, *coeff));
        }

        // We force s to be egal to our linear sum
        self.add_constraint(lin_sum);

        self.memory_s.insert(linear_sum.to_vec(), s);

        s
    }

    fn process_constraint(&mut self, linear_sum: &[(usize, IntCst)], bound_val: IntCst) {
        let opp_lin_sum = Interface::get_opposite_linear_sum(linear_sum);
        let lit: Lit;
        if self.memory_s.contains_key(linear_sum) {
            let &s = self.memory_s.get(linear_sum).unwrap();
            lit = Lit {
                var: s,
                bound: Bound::Upper,
                val: bound_val,
            };
        } else if self.memory_s.contains_key(&opp_lin_sum) {
            let &s = self.memory_s.get(&opp_lin_sum).unwrap();
            lit = Lit {
                var: s,
                bound: Bound::Lower,
                val: -bound_val,
            };
        } else {
            let s = self.add_s_var(linear_sum);

            lit = Lit {
                var: s,
                bound: Bound::Upper,
                val: bound_val,
            };
        }

        self.lit_vec.push(lit);
    }

    fn solve_iterative(&mut self) {
        let feas_check_res = self.problem.create_feasability_checker();

        debug_assert!(feas_check_res.is_ok(), "Error while creating the feasbility cheacker");

        let mut feas_check = feas_check_res.unwrap();

        let lit_vec = self.lit_vec.clone();

        for lit in lit_vec {
            let res_set_bound = feas_check.set_bound(lit.var, &lit.bound, lit.val as f64);

            self.set_bound(lit.var, lit.bound, lit.val);

            let res_check_feas = feas_check.check_feasability();

            if res_set_bound.is_err() || res_check_feas.is_err() {
                println!("Unsatisifiable constraints detected after adding {:?}", lit);

                if let Err(Error::InfeasibleWithCertificate(cert)) = &res_set_bound {
                    let is_certif_valid = self.is_certificate_valid(cert);
                    println!("Is certificate valid set_bound: {}", is_certif_valid);
                }

                if let Err(Error::InfeasibleWithCertificate(cert)) = &res_check_feas {
                    let is_certif_valid = self.is_certificate_valid(cert);
                    println!("Is certificate valid cheak_feas: {}", is_certif_valid);
                }
                break;
            }
        }
    }
}

fn main() {
    let mut interface = Interface::new();

    interface.process_constraint(&[(0, 1)], 2);
    interface.process_constraint(&[(1, 1)], 2);
    interface.process_constraint(&[(0, -1), (1, -1)], -5);

    interface.solve_iterative();

    // println!("{:?}", interface);
}

#[cfg(test)]
mod tests {
    use rand::{Rng, SeedableRng, rngs::SmallRng, seq::IteratorRandom};

    use crate::{Error, IntCst, Interface};

    #[test]
    fn opposite_linear_sum() {
        {
            let lin_sum = vec![(1, 4), (3, -1), (2, 6)];

            assert_eq!(
                Interface::get_opposite_linear_sum(&lin_sum),
                vec![(1, -4), (3, 1), (2, -6)]
            )
        }

        {
            let lin_sum = vec![];

            assert_eq!(Interface::get_opposite_linear_sum(&lin_sum), vec![])
        }
    }

    impl Interface {
        fn get_validity_certificates(&mut self) -> Option<(bool, bool)> {
            let feas_check_res = self.problem.create_feasability_checker();

            debug_assert!(feas_check_res.is_ok(), "Error while creating the feasbility cheacker");

            let mut feas_check = feas_check_res.unwrap();

            let lit_vec = self.lit_vec.clone();

            for lit in lit_vec {
                let res_set_bound = feas_check.set_bound(lit.var, &lit.bound, lit.val as f64);

                self.set_bound(lit.var, lit.bound, lit.val);
                self.problem.set_bound(lit.var, &lit.bound, lit.val as f64);

                let res_check_feas = feas_check.check_feasability();

                if res_set_bound.is_err() || res_check_feas.is_err() {
                    if let Err(Error::InfeasibleWithCertificate(cert)) = &res_set_bound {
                        // println!("Cert: {:?}", cert);

                        let is_certif_valid_float = self.problem.is_certificate_valid(cert);
                        let is_certif_valid_int = self.is_certificate_valid(cert);
                        return Some((is_certif_valid_int, is_certif_valid_float));
                    }

                    if let Err(Error::InfeasibleWithCertificate(cert)) = &res_check_feas {
                        // println!("Cert: {:?}", cert);

                        let is_certif_valid_float = self.problem.is_certificate_valid(cert);
                        let is_certif_valid_int = self.is_certificate_valid(cert);
                        return Some((is_certif_valid_int, is_certif_valid_float));
                    }

                    break;
                }
            }

            None
        }
    }

    fn get_nb_x(sparse_proportion: f32, rng: &mut SmallRng, nb_var: usize) -> usize {
        let k = 1.0 / sparse_proportion - 1.0;

        let nb_x_float = (nb_var - 1) as f32 * rng.r#gen::<f32>().powf(k); // we have a f32 in the range [0.0, nb_var - 1)

        1 + nb_x_float as usize
    }

    fn gen_filled_interface(
        nb_var: usize,
        nb_const: usize,
        min: IntCst,
        max: IntCst,
        sparse_proportion: f32,
        seed: u64,
    ) -> Interface {
        let mut interface = Interface::new();

        let mut rng = SmallRng::seed_from_u64(seed);

        for _ in 0..nb_const {
            let nb_x = get_nb_x(sparse_proportion, &mut rng, nb_var);

            let x_vec: Vec<usize> = (0..nb_var).choose_multiple(&mut rng, nb_x);

            let linear_sum: Vec<(usize, IntCst)> = x_vec.iter().map(|&x| (x, rng.gen_range(min, max))).collect();

            // println!("constraint: {:?}", linear_sum);

            let bound_val = rng.gen_range(min, max);

            interface.process_constraint(&linear_sum, bound_val);
        }

        interface
    }

    fn compile_stats_certificate(nb_var: usize, nb_const: usize, min: IntCst, max: IntCst) {
        let n = 1000;

        let mut nb_val_cert_i = 0;

        let mut nb_val_cert_f = 0;

        let mut nb_cert = 0;

        for seed in 0..n {
            let mut interface = gen_filled_interface(nb_var, nb_const, min, max, 0.1, seed);

            if let Some((is_val_cert_i, is_val_cert_f)) = interface.get_validity_certificates() {
                nb_val_cert_i += is_val_cert_i as usize;
                nb_val_cert_f += is_val_cert_f as usize;

                nb_cert += 1;
            }
        }

        println!("float: {nb_val_cert_f}, int: {nb_val_cert_i}, total: {nb_cert}");
    }

    #[test]
    fn compile_stats_certificate_single() {
        compile_stats_certificate(50, 100, -10, 10);
    }

    #[test]
    fn compile_stats_certificate_multiple() {
        for max in [10, 1000, i32::MAX] {
            for nb_var in [30, 50, 100, 150] {
                for nb_const in [50, 100, 200, 400] {
                    print!("nb_var:{nb_var}, nb_const: {nb_const}, max: {max}, ");
                    compile_stats_certificate(nb_var, nb_const, if max == i32::MAX { i32::MIN } else { -max }, max);
                }
            }
        }
    }
}
