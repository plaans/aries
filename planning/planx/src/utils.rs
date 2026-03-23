use std::fmt::{Display, Error, Formatter};

#[allow(clippy::while_let_on_iterator)]
pub fn disp_slice<T: Display>(f: &mut Formatter<'_>, iterable: &[T], sep: &str) -> Result<(), Error> {
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

#[allow(clippy::while_let_on_iterator)]
pub fn disp_iter<T: Display>(f: &mut Formatter<'_>, iterable: impl Iterator<Item = T>, sep: &str) -> Result<(), Error> {
    let mut i = iterable;
    if let Some(first) = i.next() {
        write!(f, "{first}")?;
        while let Some(other) = i.next() {
            write!(f, "{sep}")?;
            write!(f, "{other}")?;
        }
    }
    Result::Ok(())
}
