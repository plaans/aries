use crate::model::label::Label;
use crate::model::lang::{Atom, FAtom, IAtom, IVar, Kind, SAtom, Type};
use crate::model::symbols::{SymId, SymbolTable};
use crate::model::types::TypeId;
use crate::model::ModelShape;
use crate::core::*;
use crate::utils::input::Sym;
use crate::utils::Fmt;

pub trait Shaped<Lbl>
where
    Lbl: Label,
{
    fn get_shape(&self) -> &ModelShape<Lbl>;

    fn get_label(&self, var: impl Into<VarRef>) -> Option<&Lbl> {
        self.get_shape().labels.get(var.into())
    }

    fn get_type(&self, var: impl Into<VarRef>) -> Option<Type> {
        self.get_shape().types.get(var.into()).copied()
    }

    fn get_var(&self, label: &Lbl) -> Option<VarRef> {
        self.get_shape().get_variable(label)
    }

    fn get_int_var(&self, label: &Lbl) -> Option<IVar> {
        self.get_var(label).map(IVar::new)
    }

    fn get_symbol(&self, sym: SymId) -> &Sym {
        self.get_shape().symbols.symbol(sym)
    }

    fn get_type_of(&self, sym: SymId) -> TypeId {
        self.get_shape().symbols.type_of(sym)
    }

    fn get_symbol_table(&self) -> &SymbolTable {
        &self.get_shape().symbols
    }
}

/// Wraps an atom into a custom object that can be formatted with the standard library `Display`
///
/// Expressions and variables are formatted into a single line with lisp-like syntax.
/// Anonymous variables are prefixed with "b_" and "i_" (for bools and ints respectively followed
/// by a unique identifier.
///
/// # Usage
/// ```
/// use aries::model::Model;
/// use aries::model::extensions::fmt;
/// let mut i = Model::<&'static str>::default();
/// let x = i.new_ivar(0, 10, "X");
/// let y = x + 10;
/// println!("x: {}", fmt(x, &i));
/// println!("y: {}", fmt(y, &i));
/// ```
pub fn fmt<Lbl: Label>(atom: impl Into<Atom>, ctx: &impl Shaped<Lbl>) -> impl std::fmt::Display + '_ {
    let atom = atom.into();
    Fmt(move |f| format_impl(ctx, atom, f))
}

fn format_impl<Lbl: Label>(ctx: &impl Shaped<Lbl>, atom: Atom, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match atom {
        Atom::Bool(b) => format_impl_bool(ctx, b, f),
        Atom::Int(i) => format_impl_int(ctx, i, f),
        Atom::Fixed(i) => format_impl_fixed(ctx, i, f),
        Atom::Sym(s) => format_impl_sym(ctx, s, f),
    }
}
fn format_impl_bool<Lbl: Label>(ctx: &impl Shaped<Lbl>, b: Lit, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let tpe = ctx.get_type(b.variable());
    let t = b.variable().geq(1);
    if b == Lit::TRUE {
        write!(f, "true")
    } else if b == Lit::FALSE {
        write!(f, "false")
    } else if let Some(Type::Bool) = tpe {
        if b == t {
            format_impl_var(ctx, b.variable(), Kind::Bool, f)
        } else {
            debug_assert_eq!(b, !t);
            write!(f, "!")?;
            format_impl_var(ctx, b.variable(), Kind::Bool, f)
        }
    } else {
        let tpe = tpe.unwrap_or(Type::Int);
        format_impl_var(ctx, b.variable(), tpe.into(), f)?;
        write!(f, " {} {}", b.relation(), b.value())
    }
}

#[allow(clippy::comparison_chain)]
fn format_impl_int<Lbl: Label>(ctx: &impl Shaped<Lbl>, i: IAtom, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match i.var {
        IVar::ZERO => write!(f, "{}", i.shift),
        v => {
            if i.shift > 0 {
                write!(f, "(+ ")?;
            } else if i.shift < 0 {
                write!(f, "(- ")?;
            }
            format_impl_var(ctx, v.into(), Kind::Int, f)?;
            if i.shift != 0 {
                write!(f, " {})", i.shift.abs())?;
            }
            std::fmt::Result::Ok(())
        }
    }
}

#[allow(clippy::comparison_chain)]
fn format_impl_fixed<Lbl: Label>(
    ctx: &impl Shaped<Lbl>,
    i: FAtom,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    write!(f, "(/ ")?;
    format_impl_int(ctx, i.num, f)?;
    write!(f, " {})", i.denom)
}

fn format_impl_sym<Lbl: Label>(
    ctx: &impl Shaped<Lbl>,
    atom: SAtom,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    match atom {
        SAtom::Var(v) => format_impl_var(ctx, v.var, Kind::Sym, f),
        SAtom::Cst(s) => write!(f, "{}", ctx.get_symbol(s.sym)),
    }
}

fn format_impl_var<Lbl: Label>(
    ctx: &impl Shaped<Lbl>,
    v: VarRef,
    kind: Kind,
    f: &mut std::fmt::Formatter<'_>,
) -> std::fmt::Result {
    if let Some(lbl) = ctx.get_label(v) {
        write!(f, "{lbl}")
    } else {
        let prefix = match kind {
            Kind::Bool => "b_",
            Kind::Int => "i_",
            Kind::Fixed(_) => "f_",
            Kind::Sym => "s_",
        };
        write!(f, "{}{}", prefix, usize::from(v))
    }
}
