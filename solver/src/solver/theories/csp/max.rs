use crate::solver::theories::csp::{CSPView, Change, Constraint, Update};
use crate::model::lang::{IVar, IntCst, VarRef};

/// Implementation from choco : https://github.com/chocoteam/choco-solver/blob/master/solver/src/main/java/org/chocosolver/solver/constraints/nary/min_max/PropMax.java
pub struct MaxConstraint {
    lhs: IVar,
    rhs: Vec<IVar>,
}

impl MaxConstraint {
    pub fn propagate(&self, mut csp: CSPView) -> Update {
        let mut filter = true;
        while filter {
            filter = false;
            let mut lb = IntCst::MIN;
            let mut ub = IntCst::MIN;
            let max = csp.ub(self.lhs);
            // update max
            for &v in &self.rhs {
                filter |= csp.set_ub(v, max)?;
                lb = lb.max(csp.lb(v));
                ub = ub.max(csp.ub(v));
            }
            filter |= csp.set_lb(self.lhs, lb)?;
            filter |= csp.set_ub(self.lhs, ub)?;
            lb = lb.max(csp.lb(self.lhs));
            // back propagation
            let mut c = 0;
            let mut idx = 0;
            for (i, &v) in self.rhs.iter().enumerate() {
                if csp.ub(v) < lb {
                    c += 1;
                } else {
                    idx = i;
                }
            }
            if c == self.rhs.len() - 1 {
                filter = false;
                let v = self.rhs[idx];
                csp.set_lb(v, csp.lb(self.lhs))?;
                csp.set_ub(v, csp.ub(self.lhs))?;
                if csp.is_instantiated(self.lhs) {
                    csp.make_passive()
                }
            }
        }

        Ok(())
    }
}

impl Constraint for MaxConstraint {
    fn for_each_var(&self, f: &mut dyn FnMut(VarRef)) {
        f(self.lhs.into());
        for v in &self.rhs {
            f(VarRef::from(*v));
        }
    }

    fn init(&self, mut csp: CSPView) -> Update {
        csp.watch(self.lhs);
        for &v in &self.rhs {
            csp.watch(v);
        }
        self.propagate(csp)
    }

    fn propagate(&self, _changed: IVar, csp: CSPView) -> Update {
        self.propagate(csp)
    }

    fn explain_lb(&self, ivar: IVar, out: &mut Vec<Change>) {
        if ivar == self.lhs {
            todo!()
        } else {
            out.push(Change::Lb(self.lhs))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solver::theories::csp::{UpdateFail, CSP};
    use crate::model::bounds::Lit;
    use crate::model::{Model, WriterId};

    #[test]
    fn test_max() -> Result<(), UpdateFail> {
        let mut model = Model::new();
        let act = model.new_bvar("active");
        let a = model.new_ivar(0, 10, "a");
        let b = model.new_ivar(0, 9, "b");
        let c = model.new_ivar(0, 10, "c");
        let max = MaxConstraint {
            lhs: a,
            rhs: vec![b, c],
        };
        let writer = &mut model.writer(WriterId::new(0));
        let act = Lit::geq(act, 1);
        let mut csp = CSP::default();
        csp.record(act, Box::new(max));
        csp.trigger(act, writer.dup())?;

        assert_eq!(writer.bounds(a).1, 10);
        writer.set_upper_bound(c, 8, 0u32);
        csp.propagate(c, writer.dup())?;
        assert_eq!(writer.bounds(a).1, 9);

        writer.set_upper_bound(a, 7, 0u32);
        csp.propagate(a, writer.dup())?;
        assert_eq!(writer.bounds(b).1, 7);
        assert_eq!(writer.bounds(c).1, 7);

        Ok(())
    }
}
