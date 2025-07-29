/// Optimization goal: maximize or mininimize.
///
/// ```flatzinc
/// solve maximize happiness;
/// ```
#[derive(PartialEq, Debug)]
pub enum Goal {
    Maximize,
    Minimize,
}
