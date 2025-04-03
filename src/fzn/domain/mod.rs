//! Flatzinc variable domain.

mod bool_domain;
mod int_domain;
mod range;

pub use bool_domain::BoolDomain;
pub use int_domain::IntDomain;
pub use range::IntRange;
pub use range::Range;
