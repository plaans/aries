use crate::expressions::ExprHandle;
use crate::lang::{Atom, BAtom, BExpr, Expr, IAtom, IVar, Kind, SAtom, VarRef};
use crate::symbols::{SymId, SymbolTable};
use crate::types::TypeId;
use crate::ModelShape;
use aries_utils::input::Sym;
use aries_utils::Fmt;

pub trait Shaped {
    fn get_shape(&self) -> &ModelShape;

    fn get_label(&self, var: impl Into<VarRef>) -> Option<&str> {
        self.get_shape().labels.get(var.into()).map(|s| s.as_str())
    }

    fn get_symbol(&self, sym: SymId) -> &Sym {
        self.get_shape().symbols.symbol(sym)
    }

    fn get_type_of(&self, sym: SymId) -> TypeId {
        self.get_shape().symbols.type_of(sym)
    }

    fn get_expr(&self, expr: ExprHandle) -> &Expr {
        self.get_shape().expressions.get_ref(expr)
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
/// let mut i = Model::default();
/// let x = i.new_ivar(0, 10, "X");
/// let y = x + 10;
/// println!("x: {}", fmt(x, &i));
/// println!("y: {}", fmt(y, &i));
/// ```
pub fn fmt(atom: impl Into<Atom>, ctx: &impl Shaped) -> impl std::fmt::Display + '_ {
    let atom = atom.into();
    Fmt(move |f| format_impl(ctx, atom, f))
}

fn format_impl(ctx: &impl Shaped, atom: Atom, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match atom {
        Atom::Bool(b) => format_impl_bool(ctx, b, f),
        Atom::Int(i) => format_impl_int(ctx, i, f),
        Atom::Sym(s) => format_impl_sym(ctx, s, f),
    }
}
fn format_impl_bool(ctx: &impl Shaped, atom: BAtom, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match atom {
        BAtom::Cst(b) => write!(f, "{}", b),
        BAtom::Literal(b) => {
            format_impl_var(ctx, b.variable(), Kind::Int, f)?;
            write!(f, " {} {}", b.relation(), b.value())
        }
        BAtom::Expr(BExpr { expr, negated }) => {
            if negated {
                write!(f, "(not ")?;
            }
            format_impl_expr(ctx, expr, f)?;
            if negated {
                write!(f, ")")?;
            }
            Ok(())
        }
    }
}

fn format_impl_expr(ctx: &impl Shaped, expr: ExprHandle, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    let expr = ctx.get_expr(expr);
    write!(f, "({}", expr.fun)?;
    for arg in &expr.args {
        write!(f, " ")?;
        format_impl(ctx, *arg, f)?;
    }
    write!(f, ")")
}

#[allow(clippy::comparison_chain)]
fn format_impl_int(ctx: &impl Shaped, i: IAtom, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

fn format_impl_sym(ctx: &impl Shaped, atom: SAtom, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    match atom {
        SAtom::Var(v) => format_impl_var(ctx, v.var, Kind::Sym, f),
        SAtom::Cst(s) => write!(f, "{}", ctx.get_symbol(s.sym)),
    }
}

fn format_impl_var(ctx: &impl Shaped, v: VarRef, kind: Kind, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    if let Some(lbl) = ctx.get_label(v) {
        write!(f, "{}", lbl)
    } else {
        let prefix = match kind {
            Kind::Bool => "b_",
            Kind::Int => "i_",
            Kind::Sym => "s_",
        };
        write!(f, "{}{}", prefix, usize::from(v))
    }
}
