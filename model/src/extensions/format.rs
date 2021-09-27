use crate::bounds::Lit;
use crate::label::Label;
use crate::lang::{Atom, IAtom, IVar, Kind, SAtom, VarRef};
use crate::symbols::{SymId, SymbolTable};
use crate::types::TypeId;
use crate::ModelShape;
use aries_utils::input::Sym;
use aries_utils::Fmt;

pub trait Shaped<Lbl>
where
    Lbl: 'static,
{
    fn get_shape(&self) -> &ModelShape<Lbl>;

    fn get_label(&self, var: impl Into<VarRef>) -> Option<&Lbl> {
        self.get_shape().labels.get(var.into())
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
/// use aries_model::Model;
/// use aries_model::extensions::fmt;
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
        Atom::Sym(s) => format_impl_sym(ctx, s, f),
    }
}
fn format_impl_bool<Lbl: Label>(ctx: &impl Shaped<Lbl>, b: Lit, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    format_impl_var(ctx, b.variable(), Kind::Int, f)?;
    write!(f, " {} {}", b.relation(), b.value())
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
        write!(f, "{:?}", lbl)
    } else {
        let prefix = match kind {
            Kind::Bool => "b_",
            Kind::Int => "i_",
            Kind::Sym => "s_",
        };
        write!(f, "{}{}", prefix, usize::from(v))
    }
}
