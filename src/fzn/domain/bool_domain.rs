/// Boolean domain.
#[derive(PartialEq, Eq, Debug)]
pub enum BoolDomain {
    Singleton(bool),
    Both,
}
