use std::collections::HashSet;

use crate::types::*;

/// Generic set defined by its values.
/// 
/// Remark: it can be empty.
pub type Set<T> = HashSet<T>;

pub type IntSet = Set<Int>;