use crate::core::state::Term;
use crate::core::{IntCst, Lit, VarRef};
use crate::model::lang::{Atom, Cst, FAtom, IAtom, Rational, SAtom};
use crate::model::symbols::{SymId, TypedSym};
use std::collections::HashMap;

/// Extension trait to allow the evaluation of expressions based on a partial assignment of variables.
pub trait PartialAssignment {
    fn val(&self, var: VarRef) -> Option<IntCst>;

    fn evaluate(&self, atom: Atom) -> Option<Cst> {
        match atom {
            Atom::Bool(b) => self.evaluate_bool(b).map(Cst::Bool),
            Atom::Int(i) => self.evaluate_int(i).map(Cst::Int),
            Atom::Fixed(f) => self.evaluate_fixed(f).map(Cst::Fixed),
            Atom::Sym(s) => self.evaluate_sym(s).map(Cst::Sym),
        }
    }

    fn evaluate_seq(&self, atoms: &[Atom]) -> Option<Vec<Cst>> {
        let mut res = Vec::with_capacity(atoms.len());
        for a in atoms {
            res.push(self.evaluate(*a)?);
        }
        Some(res)
    }

    fn evaluate_bool(&self, lit: Lit) -> Option<bool> {
        match self.val(lit.variable()) {
            None => None,
            Some(i) => {
                if lit.svar().is_plus() {
                    Some(i <= lit.value())
                } else {
                    Some(-i <= lit.value())
                }
            }
        }
    }

    fn evaluate_int(&self, iatom: IAtom) -> Option<IntCst> {
        self.val(iatom.var.variable()).map(|i| i + iatom.shift)
    }

    fn evaluate_fixed(&self, e: FAtom) -> Option<Rational> {
        self.evaluate_int(e.num).map(|num| Rational::new(num, e.denom))
    }

    fn evaluate_sym(&self, satom: SAtom) -> Option<TypedSym> {
        match satom {
            SAtom::Var(v) => self
                .val(v.variable())
                .map(|i| TypedSym::new(SymId::from(i as usize), v.tpe)),
            SAtom::Cst(c) => Some(c),
        }
    }
}

/// A tool to construct a partial assignment by binding expressions (Atom) to their values (Cst)
pub struct PartialAssignmentBuilder {
    values: HashMap<VarRef, IntCst>,
}

#[derive(Debug, Copy, Clone)]
pub struct InvalidAssignment;

impl PartialAssignmentBuilder {
    pub fn new() -> Self {
        let mut s = Self {
            values: Default::default(),
        };
        s.add_var(VarRef::ZERO, 0).unwrap();
        s.add_var(VarRef::ONE, 1).unwrap();
        s
    }

    fn add_var(&mut self, var: VarRef, val: IntCst) -> Result<(), InvalidAssignment> {
        if let Some(prev) = self.values.get(&var).copied() {
            if val == prev {
                Ok(())
            } else {
                Err(InvalidAssignment)
            }
        } else {
            self.values.insert(var, val);
            Ok(())
        }
    }

    pub fn add(&mut self, atom: impl Into<Atom>, value: impl Into<Cst>) -> Result<(), InvalidAssignment> {
        let atom = atom.into();
        let value = value.into();
        match (atom, value) {
            (Atom::Bool(e), Cst::Bool(v)) => self.add_bool(e, v),
            (Atom::Int(e), Cst::Int(v)) => self.add_int(e, v),
            (Atom::Fixed(e), Cst::Fixed(v)) => self.add_fixed(e, v),
            (Atom::Sym(e), Cst::Sym(v)) => self.add_sym(e, v),
            _ => Err(InvalidAssignment),
        }
    }

    pub fn add_int(&mut self, ai: IAtom, i: IntCst) -> Result<(), InvalidAssignment> {
        let var = ai.var.variable();
        let value = i - ai.shift;
        self.add_var(var, value)
    }

    pub fn add_sym(&mut self, expr: SAtom, val: TypedSym) -> Result<(), InvalidAssignment> {
        match expr {
            SAtom::Var(v) => self.add_var(v.var, val.sym.int_value()),
            SAtom::Cst(cst) if cst.sym == val.sym => Ok(()),
            _ => Err(InvalidAssignment),
        }
    }
    #[allow(clippy::collapsible_else_if)]
    pub fn add_bool(&mut self, e: Lit, val: bool) -> Result<(), InvalidAssignment> {
        let ub = e.value();
        if val {
            if e.svar().is_plus() {
                // v <= ub
                self.add_var(e.variable(), ub)
            } else {
                // -v <= ub   <=>  v >= -ub +1
                self.add_var(e.variable(), -ub + 1)
            }
        } else {
            if e.svar().is_plus() {
                // v > ub
                self.add_var(e.variable(), ub + 1)
            } else {
                // -v > ub   <=>  v <= -ub -1
                self.add_var(e.variable(), -ub - 1)
            }
        }
    }

    pub fn add_fixed(&mut self, e: FAtom, v: Rational) -> Result<(), InvalidAssignment> {
        let int_value = v * e.denom;
        if !int_value.is_integer() {
            return Err(InvalidAssignment);
        }
        let int_value = int_value.to_integer();
        self.add_int(e.num, int_value)
    }
}

impl PartialAssignment for PartialAssignmentBuilder {
    fn val(&self, var: VarRef) -> Option<IntCst> {
        self.values.get(&var).copied()
    }
}

impl Default for PartialAssignmentBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use crate::model::extensions::partial_assignment::{PartialAssignment, PartialAssignmentBuilder};
    use crate::model::lang::{Atom, Cst};
    use crate::model::Model;

    #[test]
    fn test_partial_assignment() {
        fn val(ass: &PartialAssignmentBuilder, a: impl Into<Atom>, val: impl Into<Cst>) {
            assert_eq!(ass.evaluate(a.into()), Some(val.into()));
        }
        fn undef(ass: &PartialAssignmentBuilder, a: impl Into<Atom>) {
            assert_eq!(ass.evaluate(a.into()), None);
        }
        let mut m: Model<&'static str> = Model::new();
        let a = m.new_ivar(0, 10, "a");
        let b = m.new_ivar(0, 10, "b");

        let ass = &mut PartialAssignmentBuilder::new();
        undef(ass, a);
        undef(ass, b);

        ass.add(a, 3).unwrap();
        val(ass, a, 3);
        val(ass, a + 1, 4);
        val(ass, a - 2, 1);
        undef(ass, b);

        assert!(ass.add(a, 3).is_ok());
        assert!(ass.add(a, 0).is_err());
    }
}
