use crate::chronicles::Problem;
use std::collections::HashSet;

pub type FeatureSet = HashSet<Feature>;

#[derive(Ord, PartialOrd, Eq, PartialEq, Hash, Debug, Copy, Clone)]
pub enum Feature {
    /// The problem features numeric fluents
    Numeric,
}

pub fn features(pb: &Problem) -> FeatureSet {
    let mut features = HashSet::new();
    for fluent in &pb.context.fluents {
        if fluent.return_type().is_numeric() {
            features.insert(Feature::Numeric);
        }
    }
    features
}
