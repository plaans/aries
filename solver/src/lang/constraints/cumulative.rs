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

                let starts_before = leq(si, pj.start).reified(ctx);
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
