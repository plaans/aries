use std::fmt::{Display, Error, Formatter};

/// A custom type to extract the formatter and feed it to formal_impl
/// Source: `<https://github.com/rust-lang/rust/issues/46591#issuecomment-350437057>`
pub struct Fmt<F>(pub F)
where
    F: Fn(&mut std::fmt::Formatter) -> std::fmt::Result;

impl<F> std::fmt::Display for Fmt<F>
where
    F: Fn(&mut std::fmt::Formatter) -> std::fmt::Result,
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        (self.0)(f)
    }
}

#[allow(clippy::while_let_on_iterator)]
pub fn disp_iter<T: Display>(f: &mut Formatter<'_>, iterable: &[T], sep: &str) -> Result<(), Error> {
    let mut i = iterable.iter();
    if let Some(first) = i.next() {
        write!(f, "{first}")?;
        while let Some(other) = i.next() {
            write!(f, "{sep}")?;
            write!(f, "{other}")?;
        }
    }
    Result::Ok(())
}
