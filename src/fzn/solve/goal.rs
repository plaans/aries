/// Optimization goal: maximize or mininimize.
///
/// ```flatzinc
/// solve maximize happyness;
/// ```
#[derive(PartialEq, Debug)]
pub enum Goal {
    Maximize,
    Minimize,
}
