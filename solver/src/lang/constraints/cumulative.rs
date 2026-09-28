use crate::prelude::*;

/// Indicates a consumption of a resource over the interval `[start,end)`.
/// The consumption only occurs when `present` is true.
pub struct Pulse {
    pub present: Lit,
    pub start: VarCst,
    pub end: VarCst,
    pub consumption: IntCst,
}

/// Imposes a maximum consumption over a resource
pub struct Cumulative {
    capacity: IntCst,
    pulses: Vec<Pulse>,
}

impl Cumulative {
    pub fn new(capacity: IntCst, pulses: Vec<Pulse>) -> Self {
        Self { capacity, pulses }
    }
}

impl<Ctx: ModelView> BoolExpr<Ctx> for Cumulative {
    fn enforce_if(&self, implicant: Lit, ctx: &mut Ctx) {
        for (i, pi) in self.pulses.iter().enumerate() {
            let si = pi.start;

            let mut consumed_at_start = LinSum::cst(pi.consumption);

            for (j, pj) in self.pulses.iter().enumerate() {
                if i == j {
                    continue;
                }

                // pulse j contributes at the start of pulse i if it is active at that time: sj <= si < ej
                let starts_before = leq(pj.start, si).reified(ctx);
                let ends_after = gt(pj.end, si).reified(ctx);

                let contributes = and([pj.present, starts_before, ends_after]).reified(ctx);
                consumed_at_start += bool2int(contributes, ctx) * pj.consumption;
            }

            consumed_at_start
                .leq(self.capacity)
                .scoped(pi.present)
                .opt_enforce_if(implicant, ctx);
        }
    }

    fn conj_scope(&self, _ctx: &Ctx) -> Conjunction {
        Lit::TRUE.into()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    /// Builds a model with the given pulses posted on a resource of the given capacity.
    fn model_with(capacity: IntCst, pulses: Vec<(IntCst, IntCst, IntCst)>) -> Model {
        let mut m = Model::new();
        let mut variables = Vec::new();
        for (i, (start, end, consumption)) in pulses.into_iter().enumerate() {
            let s = m.new_ivar(0, 20, i.to_string());
            m.enforce(eq(s, start));
            m.enforce(eq(s + (end - start), end));
            variables.push(Pulse {
                present: Lit::TRUE,
                start: s.into(),
                end: s + (end - start),
                consumption,
            });
        }
        m.enforce(Cumulative::new(capacity, variables));
        m
    }

    fn is_satisfiable(m: Model) -> bool {
        let mut solver = Solver::new(m);
        solver.solve(SearchLimit::None).unwrap().is_some()
    }

    /// The last task starts exactly when the two overlapping ones are finished: feasible.
    #[test]
    fn test_sequential_tasks() {
        let m = model_with(2, vec![(0, 6, 1), (0, 6, 1), (6, 10, 1)]);
        assert!(is_satisfiable(m));
    }

    /// Two overlapping tasks whose consumptions exceed the capacity: infeasible.
    #[test]
    fn test_overlapping_tasks() {
        let m = model_with(3, vec![(0, 6, 2), (1, 7, 2)]);
        assert!(!is_satisfiable(m));
    }

    /// Overlapping tasks whose consumptions fit within the capacity: feasible.
    #[test]
    fn test_parallel_tasks() {
        let m = model_with(3, vec![(0, 6, 2), (1, 7, 1)]);
        assert!(is_satisfiable(m));
    }
}
