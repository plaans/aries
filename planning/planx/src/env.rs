use idmap::DirectIdMap;

use crate::{Expr, ExprId, ExprNode, Fluents, Message, Objects, Types, errors::Span};

pub struct Environment {
    pub types: Types,
    pub objects: Objects,
    pub fluents: Fluents,
    exprs: DirectIdMap<ExprId, ExprNode>,
    next_expr_id: u32,
}

#[derive(Copy, Clone)]
pub struct Env<'a, T> {
    pub elem: T,
    pub env: &'a Environment,
}

impl Environment {
    pub fn new(types: Types) -> Self {
        Self {
            types,
            objects: Default::default(),
            fluents: Default::default(),
            exprs: Default::default(),
            next_expr_id: 0u32,
        }
    }
    pub(super) fn get(&self, id: ExprId) -> &ExprNode {
        self.exprs.get(id).unwrap()
    }

    pub fn node<T>(&self, id: T) -> Env<'_, T> {
        self / id
    }

    pub fn intern(&mut self, expr: Expr, span: impl Into<Option<Span>>) -> Result<ExprId, Message> {
        let tpe = expr.tpe(self)?;
        let id = ExprId(self.next_expr_id);
        self.next_expr_id += 1;
        let res = self.exprs.insert(id, ExprNode::new(expr, tpe, span.into()));
        debug_assert!(res.is_none());
        Ok(id)
    }
}

impl<'a, T> std::ops::Div<T> for &'a Environment {
    type Output = Env<'a, T>;

    fn div(self, rhs: T) -> Self::Output {
        Env { elem: rhs, env: self }
    }
}

impl<'a, T> std::ops::Div<T> for &'a mut Environment {
    type Output = Env<'a, T>;

    fn div(self, rhs: T) -> Self::Output {
        Env { elem: rhs, env: self }
    }
}

/// Specialization of [`format_section`] for top level elements
pub(crate) fn fstop<'a, T: 'a>(
    f: &mut std::fmt::Formatter<'_>,
    name: &str,
    free_line_between: bool,
    xs: impl IntoIterator<Item = T>,
    env: &'a Environment,
) -> std::fmt::Result
where
    Env<'a, T>: std::fmt::Display,
{
    format_section(f, name, 0, true, free_line_between, xs, env)
}

/// Specialization of [`format_section`] for nested elements
pub(crate) fn fs<'a, T: 'a>(
    f: &mut std::fmt::Formatter<'_>,
    name: &str,
    xs: impl IntoIterator<Item = T>,
    env: &'a Environment,
) -> std::fmt::Result
where
    Env<'a, T>: std::fmt::Display,
{
    format_section(f, name, 2, false, false, xs, env)
}

pub(crate) const INDENT: &str = "    ";

/// Helper function to format a section with a list of elements.
/// Each of the elements must be displayed when wrapper in [`Env`].
/// If there is no element the section is ommited
///
/// ```txt
///    name:
///      elem1
///      elem2
/// ```
pub(crate) fn format_section<'a, T: 'a>(
    f: &mut std::fmt::Formatter<'_>,
    name: &str,
    indent: usize,
    free_line_before: bool,
    free_line_between: bool,
    xs: impl IntoIterator<Item = T>,
    env: &'a Environment,
) -> std::fmt::Result
where
    Env<'a, T>: std::fmt::Display,
{
    let xs = xs.into_iter();
    let mut is_first = true;
    for x in xs {
        if is_first {
            if free_line_before {
                writeln!(f)?;
            }
            write!(f, "\n{}{name}", INDENT.repeat(indent))?;
            is_first = false;
        } else if free_line_between {
            writeln!(f)?;
        }
        let x = env / x;
        write!(f, "\n{}{}", INDENT.repeat(indent + 1), x)?;
    }
    Ok(())
}
