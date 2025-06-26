/* ========================================================================== */
/*                                CSP Variable                                */
/* ========================================================================== */

use std::{cmp::min, collections::HashMap, fmt::Display, mem::swap};

use anyhow::{bail, Result};
use aries::{
    core::{IntCst, Lit, INT_CST_MAX, INT_CST_MIN},
    model::{
        lang::{
            expr::{and, eq, geq, lt, or},
            IVar,
        },
        Model,
    },
    solver::{SearchLimit, Solver},
};
use malachite::{Natural, Rational};

use crate::print_info;

/// Represents a variable in a CSP problem.
#[derive(Clone, PartialEq, Eq)]
pub struct CspVariable {
    /// The domain is the union of the tuples [lb, ub[.
    domain: Vec<(Rational, Rational)>,
}

impl CspVariable {
    pub fn new(domain: Vec<(Rational, Rational)>) -> Self {
        Self { domain }
    }
}

impl Display for CspVariable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut domain = self.domain.clone();
        domain.sort();
        f.write_fmt(format_args!(
            "{}",
            domain
                .iter()
                .map(|v| format!("[{}, {}[", v.0, v.1))
                .collect::<Vec<_>>()
                .join(" U ")
        ))
    }
}

/* ========================================================================== */
/*                               CSP Constraint                               */
/* ========================================================================== */

/// Represents the term of a constraint in a CSP problem.
#[derive(Clone)]
pub struct CspConstraintTerm {
    id: String,
    delay: Rational,
}

impl CspConstraintTerm {
    pub fn id(&self) -> &String {
        &self.id
    }

    pub fn new_delayed(id: String, delay: Rational) -> Self {
        Self { id, delay }
    }

    pub fn new(id: String) -> Self {
        Self { id, delay: 0.into() }
    }
}

/// Represents a constraint in a CSP problem.
#[derive(Clone)]
pub enum CspConstraint {
    Lt(CspConstraintTerm, CspConstraintTerm),     // a < b
    Le(CspConstraintTerm, CspConstraintTerm),     // a <= b
    Equals(CspConstraintTerm, CspConstraintTerm), // a == b
    Not(Box<CspConstraint>),                      // not (a)
    Or(Vec<CspConstraint>),                       // a1 or a2 or ... or an
}

impl Display for CspConstraintTerm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("{}", self.id))?;
        if self.delay != 0 {
            let s = if self.delay < 0 { "-" } else { "+" };
            f.write_fmt(format_args!(" {} {}", s, self.delay))?;
        }
        Ok(())
    }
}

impl Display for CspConstraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CspConstraint::Lt(a, b) => f.write_fmt(format_args!("{a} <  {b}")),
            CspConstraint::Le(a, b) => f.write_fmt(format_args!("{a} <= {b}")),
            CspConstraint::Equals(a, b) => f.write_fmt(format_args!("{a} == {b}")),
            CspConstraint::Not(a) => f.write_fmt(format_args!("not ({a})")),
            CspConstraint::Or(a) => f.write_fmt(format_args!(
                "{}",
                a.iter().map(|c| format!("({c})")).collect::<Vec<_>>().join(" or ")
            )),
        }
    }
}

/* ========================================================================== */
/*                                 CSP Problem                                */
/* ========================================================================== */

/// Represents a CSP problem.
#[derive(Default)]
pub struct CspProblem {
    variables: HashMap<String, CspVariable>,
    constraints: Vec<CspConstraint>,
    cached_lcm: Option<IntCst>,
}

impl CspProblem {
    /// Returns all CspConstraintTerm contained in the problem.
    fn constraint_terms(&self) -> Vec<&CspConstraintTerm> {
        fn extract_from(c: &CspConstraint) -> Vec<&CspConstraintTerm> {
            match c {
                CspConstraint::Lt(lhs, rhs) => vec![lhs, rhs],
                CspConstraint::Le(lhs, rhs) => vec![lhs, rhs],
                CspConstraint::Equals(lhs, rhs) => vec![lhs, rhs],
                CspConstraint::Not(constr) => extract_from(constr),
                CspConstraint::Or(disjuncts) => disjuncts.iter().fold(vec![], |mut r, x| {
                    r.extend(extract_from(x));
                    r
                }),
            }
        }

        self.constraints.iter().fold(vec![], |mut r, c| {
            r.extend(extract_from(c));
            r
        })
    }

    /// Returns the cached lcm and calculate it if needed.
    fn lcm(&mut self) -> IntCst {
        if self.cached_lcm.is_none() {
            let mut denom = 1;
            for (_, var) in self.variables.iter() {
                for (lb, ub) in var.domain.iter() {
                    denom = lcm(denom, natural_into_cst(lb.to_denominator()));
                    denom = lcm(denom, natural_into_cst(ub.to_denominator()));
                }
            }
            for &t in self.constraint_terms().iter() {
                denom = lcm(denom, natural_into_cst(t.delay.to_denominator()));
            }
            self.cached_lcm = Some(denom);
        }
        self.cached_lcm.unwrap()
    }

    /// Normalize the rational based on the current lcm.
    fn normalize(&mut self, r: &Rational) -> IntCst {
        IntCst::saturating_mul(
            natural_into_cst(r.to_numerator()),
            self.lcm() / natural_into_cst(r.to_denominator()),
        )
        .clamp(INT_CST_MIN, INT_CST_MAX)
    }

    /// Reifies the given constraint into the given model.
    fn reify_constraint(&mut self, m: &mut Model<String>, c: &CspConstraint, vars: &HashMap<String, IVar>) -> Lit {
        let mut t = |x: &CspConstraintTerm| {
            let var = vars[&x.id];
            var + self.normalize(&x.delay)
        };

        match c {
            CspConstraint::Lt(lhs, rhs) => m.reify(lt(t(lhs), t(rhs))),
            CspConstraint::Le(lhs, rhs) => m.reify(lt(t(lhs), t(rhs) + 1)),
            CspConstraint::Equals(lhs, rhs) => m.reify(eq(t(lhs), t(rhs))),
            CspConstraint::Not(constr) => {
                let r = self.reify_constraint(m, constr, vars);
                m.reify(eq(r, Lit::FALSE))
            }
            CspConstraint::Or(disjuncts) => {
                let r = or(disjuncts
                    .iter()
                    .map(|x| self.reify_constraint(m, x, vars))
                    .collect::<Vec<_>>());
                m.reify(r)
            }
        }
    }

    /// Appends a new variable in the problem.
    pub fn add_variable(&mut self, id: String, variable: CspVariable) -> Result<()> {
        if self.variables.contains_key(&id) {
            if self.variables.get(&id).unwrap() != &variable {
                bail!(format!("The variable {id} is already assigned with another value"));
            } // Else, the values are the same so we ignore it.
        } else {
            self.variables.entry(id).or_insert(variable);
        }
        self.cached_lcm = None;
        Ok(())
    }

    /// Appends a new constraint in the problem.
    pub fn add_constraint(&mut self, constraint: CspConstraint) {
        self.constraints.push(constraint);
        self.cached_lcm = None;
    }

    /// Maps the constraints of the problem with the given function.
    pub fn map_constraints<F>(&mut self, f: F)
    where
        F: FnMut(&CspConstraint) -> CspConstraint,
    {
        self.constraints = self.constraints.iter().map(f).collect();
    }

    /// Returns the formatted id for a start variable.
    pub fn start_id(id: &String) -> String {
        format!("{id}.start")
    }

    /// Returns the formatted id for an end variable.
    pub fn end_id(id: &String) -> String {
        format!("{id}.end")
    }

    /// Returns whether the problem is valid.
    pub fn is_valid(&mut self) -> bool {
        let mut m = Model::<String>::new();
        let mut vars = HashMap::new();

        for (name, var) in self.variables.clone().iter() {
            let domain = &var.domain;
            let var = m.new_ivar(0, INT_CST_MAX, name.clone());
            vars.insert(name.clone(), var);

            let mut options = Vec::new();
            for (lb, ub) in domain {
                let lb = self.normalize(lb);
                let ub = self.normalize(ub);
                let lb = m.reify(geq(var, lb));
                let ub = m.reify(lt(var, ub));
                let in_interval = m.reify(and([lb, ub]));
                options.push(in_interval);
            }
            if !options.is_empty() {
                m.enforce(or(options), []);
            }
        }

        for c in self.constraints.clone().iter() {
            let r = self.reify_constraint(&mut m, c, &vars);
            m.enforce(r, []);
        }

        let mut solver = Solver::new(m);
        let result = solver.solve(SearchLimit::None).expect("Solver interrupted");
        result.is_some() // An assignment exists
    }
}

impl Display for CspProblem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("\n========== CSP ==========\n")?;
        f.write_str("Variables:\n")?;
        for (id, var) in self.variables.iter() {
            f.write_fmt(format_args!("    {id} in {var}\n"))?;
        }
        f.write_str("\nConstraints:\n")?;
        for constraint in self.constraints.iter() {
            f.write_fmt(format_args!("    {constraint}\n"))?;
        }
        f.write_str("=========================\n")
    }
}

/* ========================================================================== */
/*                                    Utils                                   */
/* ========================================================================== */

/// Returns the greatest common divisor.
fn gcd(a: IntCst, b: IntCst) -> IntCst {
    if a == 0 {
        return b;
    }
    if b == 0 {
        return a;
    }

    let mut u = a;
    let mut v = b;
    let i = u.trailing_zeros();
    u >>= i;
    let j = v.trailing_zeros();
    v >>= j;
    let k = min(i, j);

    loop {
        debug_assert!(u % 2 == 1);
        debug_assert!(v % 2 == 1);

        if u > v {
            swap(&mut u, &mut v);
        }

        v -= u;

        if v == 0 {
            return u << k;
        }

        v >>= v.trailing_zeros();
    }
}

/// Returns the least common multiplier.
fn lcm(a: IntCst, b: IntCst) -> IntCst {
    b * (a / gcd(a, b))
}

/// Converts a natural into IntCst.
fn natural_into_cst(n: Natural) -> IntCst {
    print_info!(false, "Converting {n} into IntCst. String is '{}'", n.to_string());
    n.to_string().parse::<IntCst>().unwrap()
}

/* ========================================================================== */
/*                                    Tests                                   */
/* ========================================================================== */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcd() {
        assert_eq!(gcd(3723, 6711), 3);
        assert_eq!(gcd(12, 8), 4);
        assert_eq!(gcd(3, 7), 1);
        assert_eq!(gcd(12, 6), 6);
        assert_eq!(gcd(10, 15), 5);
        assert_eq!(gcd(6209, 4435), 887);
        assert_eq!(gcd(1183, 455), 91)
    }

    #[test]
    fn test_lcm() {
        assert_eq!(lcm(30, 36), 180);
        assert_eq!(lcm(1, 10), 10);
        assert_eq!(lcm(33, 12), 132);
        assert_eq!(lcm(27, 48), 432);
        assert_eq!(lcm(17, 510), 510);
        assert_eq!(lcm(14, 18), 126);
        assert_eq!(lcm(39, 45), 585);
        assert_eq!(lcm(39, 130), 390);
        assert_eq!(lcm(28, 77), 308);
    }
}
