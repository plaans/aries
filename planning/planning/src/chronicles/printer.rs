#![allow(clippy::comparison_chain)]
use crate::chronicles::constraints::{Constraint, ConstraintType, Duration};
use crate::chronicles::{Chronicle, ChronicleKind, EffectOp, Problem, StateVar, Time, VarLabel, VarType};
use aries::core::{Lit, Relation, VarRef};
use aries::model::extensions::AssignmentExt;
use aries::model::lang::linear::{LinearSum, LinearTerm};
use aries::model::lang::{Atom, BVar, IAtom, IVar, SAtom};
use aries::model::symbols::SymId;
use aries::model::Model;

pub struct Printer<'a> {
    model: &'a Model<VarLabel>,
}

impl<'a> Printer<'a> {
    /// Prints the problem's chronicles to standard output (template and non-template)
    pub fn print_problem(pb: &Problem) {
        for c in &pb.chronicles {
            Self::print_chronicle(&c.chronicle, &pb.context.model);
        }
        for c in &pb.templates {
            Self::print_chronicle(&c.chronicle, &pb.context.model);
        }
    }

    pub fn print_chronicle(ch: &Chronicle, model: &Model<VarLabel>) {
        let printer = Printer { model };
        printer.chronicle(ch)
    }

    fn chronicle(&self, ch: &Chronicle) {
        match ch.kind {
            ChronicleKind::Problem => print!("problem "),
            ChronicleKind::Method => print!("method "),
            ChronicleKind::Action => print!("action "),
            ChronicleKind::DurativeAction => print!("action "),
        }
        self.list(&ch.name);
        println!();
        print!("  presence: ");
        self.var(ch.presence.variable());
        println!();

        if let Some(task) = &ch.task {
            print!("  task: ");
            self.list(task);
            println!();
        }

        println!("  conditions:");
        for c in &ch.conditions {
            print!("    [");
            self.time(c.start);
            if c.start != c.end {
                print!(", ");
                self.time(c.end);
            }
            print!("] ");
            self.sv(&c.state_var);
            print!(" == ");
            self.atom(c.value);
            println!()
        }

        println!("  effects:");
        for e in &ch.effects {
            print!("    [");
            self.time(e.transition_start);
            if e.transition_start != e.persistence_start {
                print!(", ");
                self.time(e.persistence_start);
            }
            print!("] ");
            self.sv(&e.state_var);
            self.effect_op(&e.operation);
            if !e.min_persistence_end.is_empty() {
                print!("       min-persist: ");
                self.list(&e.min_persistence_end);
            }
            println!()
        }

        println!("  constraints:");
        for c in &ch.constraints {
            print!("    ");
            self.constraint(c);
            println!();
        }

        println!("  subtasks:");
        for st in &ch.subtasks {
            print!("    [");
            self.time(st.start);
            print!(", ");
            self.time(st.end);
            print!("] ");
            self.list(&st.task_name);
            println!()
        }

        if let Some(cost) = ch.cost {
            println!("  cost: {cost}")
        }

        println!()
    }

    fn list(&self, l: &[impl Into<Atom> + Copy]) {
        for e in l {
            let a: Atom = (*e).into();
            self.atom(a);
            print!(" ");
        }
    }

    fn sv(&self, sv: &StateVar) {
        self.sym(sv.fluent.sym);
        print!(" ");
        self.list(&sv.args);
    }

    fn time(&self, t: Time) {
        let i = t.num;
        self.var(i.var.into());
        if i.shift > 0 {
            print!(" + {}", i.shift as f32 / t.denom as f32);
        } else if i.shift < 0 {
            print!(" - {}", -i.shift as f32 / t.denom as f32);
        }
    }

    fn atom(&self, a: Atom) {
        match a {
            Atom::Bool(lit) => self.lit(lit),
            Atom::Int(i) => self.iatom(i),
            Atom::Fixed(f) => self.time(f),
            Atom::Sym(s) => self.satom(s),
        }
    }

    fn effect_op(&self, op: &EffectOp) {
        match op {
            EffectOp::Assign(value) => {
                print!(" := ");
                self.atom(*value)
            }
            EffectOp::Increase(i) => {
                print!(" += {i}")
            }
        }
    }

    fn iatom(&self, i: IAtom) {
        if i.var == IVar::ZERO {
            print!("{}", i.shift)
        } else {
            self.var(i.var.into());
            if i.shift > 0 {
                print!(" + {}", i.shift);
            } else if i.shift < 0 {
                print!(" - {}", -i.shift);
            }
        }
    }

    fn satom(&self, s: SAtom) {
        match s {
            SAtom::Var(v) => self.var(v.var),
            SAtom::Cst(c) => self.sym(c.sym),
        }
    }
    fn sym(&self, s: SymId) {
        print!("{}", self.model.shape.symbols.symbol(s))
    }

    fn lit(&self, l: Lit) {
        match l {
            Lit::TRUE => print!("true"),
            Lit::FALSE => print!("false"),
            _ => {
                let (var, rel, val) = l.unpack();
                if rel == Relation::Gt && val == 0 {
                    self.var(var);
                } else if rel == Relation::Leq && val == 0 {
                    print!("!");
                    self.var(var);
                } else {
                    self.var(var);
                    print!(" {rel} {val}")
                }
            }
        }
    }

    fn var(&self, v: VarRef) {
        if let Some(VarLabel(_container, tpe)) = self.model.shape.labels.get(v) {
            match tpe {
                VarType::Horizon => print!("horizon"),
                VarType::Presence => print!("{:?}", BVar::new(v).true_lit()),
                VarType::ChronicleStart => print!("start"),
                VarType::ChronicleEnd => print!("end"),
                VarType::EffectEnd => print!("eff_end_{v:?}"),
                VarType::TaskStart(i) => print!("ts({i})"),
                VarType::TaskEnd(i) => print!("te({i})"),
                VarType::Parameter(name) => print!("{name}"),
                VarType::Reification => print!("reif_{v:?}"),
                VarType::Cost => print!("cost_{v:?}"),
            }
        } else if v == VarRef::ZERO {
            print!("0");
        } else {
            print!("{v:?}");
        }

        let prez = self.model.presence_literal(v);
        if prez != Lit::TRUE {
            print!("[{prez:?}]")
        }
    }

    fn linear_term(&self, term: &LinearTerm) {
        if term.factor() != 1 {
            print!("{}*", term.factor());
        }
        if let Some(var) = term.var() {
            self.var(var.into());
        } else {
            print!("1");
        }
    }

    fn linear_sum(&self, sum: &LinearSum) {
        let mut first = true;
        for term in sum.terms().iter() {
            if !first {
                print!(" + ");
            } else {
                first = false;
            }
            self.linear_term(term);
        }

        if sum.get_constant() != 0 {
            if !sum.terms().is_empty() {
                print!(" + ");
            }
            print!("{}", sum.get_constant())
        }
    }

    fn constraint(&self, c: &Constraint) {
        if let Some(value) = c.value {
            self.lit(value);
            print!(" == ");
        }
        print!("(");
        match &c.tpe {
            ConstraintType::InTable(table) => {
                print!("{}", table.name)
            }
            ConstraintType::Lt => {
                print!("<")
            }
            ConstraintType::Eq => {
                print!("=")
            }
            ConstraintType::Neq => {
                print!("!=")
            }
            ConstraintType::Duration(dur) => {
                print!("duration = ");
                match dur {
                    Duration::Fixed(d) => self.linear_sum(d),
                    Duration::Bounded { lb, ub } => {
                        print!("[");
                        self.linear_sum(lb);
                        print!(", ");
                        self.linear_sum(ub);
                        print!("]");
                    }
                }
            }
            ConstraintType::Or => {
                print!("or")
            }
        }
        print!(" ");
        self.list(&c.variables);
        print!(")");
    }
}
