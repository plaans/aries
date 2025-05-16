/// Boolean domain.
///
/// ```flatzinc
/// var bool: b;
/// ```
#[derive(PartialEq, Eq, Debug)]
pub enum BoolDomain {
    Singleton(bool),
    Both,
}
