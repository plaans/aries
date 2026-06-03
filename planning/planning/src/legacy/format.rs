use crate::chronicles::{Ctx, VarLabel};
use crate::legacy::input::Sym;
use crate::legacy::utils::Fmt;
use crate::legacy::*;
use aries::core::*;
use aries::lang::IAtom;
use aries::model::Label;
use aries::model::ModelShape;

pub trait Shaped<Lbl>
where
    Lbl: Label,
{
    fn get_shape(&self) -> &ModelShape<Lbl>;

    fn get_label(&self, var: impl Into<Var>) -> Option<&Lbl> {
        self.get_shape().get_label(var.into())
    }

    fn get_var(&self, label: &Lbl) -> Option<Var> {
        self.get_shape().get_variable(label)
    }

    fn get_int_var(&self, label: &Lbl) -> Option<Var> {
        self.get_var(label)
    }

    fn get_symbol(&self, sym: SymId) -> &Sym {
        self.get_symbol_table().symbol(sym)
    }

    fn get_type_of(&self, sym: SymId) -> TypeId {
        self.get_symbol_table().type_of(sym)
    }

    fn get_symbol_table(&self) -> &SymbolTable;
}

impl Shaped<VarLabel> for Ctx {
    fn get_shape(&self) -> &ModelShape<VarLabel> {
        &self.model.shape
    }

    fn get_symbol_table(&self) -> &SymbolTable {
        self.symbols.as_ref()
    }
}

// impl<Lbl: Label> Shaped<Lbl> for Model<Lbl> {
//     fn get_shape(&self) -> &ModelShape<Lbl> {
//         &self.shape
//     }
// }
// impl<Lbl: Label> Shaped<Lbl> for Solver<Lbl> {
//     fn get_shape(&self) -> &ModelShape<Lbl> {
//         self.model.get_shape()
//     }
// }

/// Wraps an atom into a custom object that can be formatted with the standard library `Display`
///
/// Expressions and variables are formatted into a single line with lisp-like syntax.
/// Anonymous variables are prefixed with "b_" and "i_" (for bools and ints respectively followed
/// by a unique identifier.
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
    if b == Lit::TRUE {
        write!(f, "true")
    } else if b == Lit::FALSE {
        write!(f, "false")
    } else {
        format_impl_var(ctx, b.variable(), Kind::Int, f)?;
        if b.svar().is_plus() {
            write!(f, " <= {}", b.ub_value())
        } else {
            write!(f, " >= {}", -b.ub_value())
        }
    }
}

#[allow(clippy::comparison_chain)]
fn format_impl_int<Lbl: Label>(ctx: &impl Shaped<Lbl>, i: IAtom, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match i.var {
        Var::ZERO => write!(f, "{}", i.shift),
        v => {
            if i.shift > 0 {
                write!(f, "(+ ")?;
            } else if i.shift < 0 {
                write!(f, "(- ")?;
            }
            format_impl_var(ctx, v, Kind::Int, f)?;
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
    v: Var,
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
