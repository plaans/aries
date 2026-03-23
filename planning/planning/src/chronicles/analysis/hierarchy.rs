use crate::chronicles::analysis::ProblemClass;
use crate::chronicles::{Chronicle, Problem};
use aries::model::extensions::DomainsExt;
use aries::model::lang::Atom;

pub fn class_of(pb: &Problem) -> ProblemClass {
    // TODO: this is an approximation as a hierarchical problem where no subtasks appears would be considered flat
    if is_hierarchical(pb) {
        if hierarchical_is_non_recursive(pb) {
            ProblemClass::HierarchicalNonRecursive
        } else {
            ProblemClass::HierarchicalRecursive
        }
    } else if pb.templates.is_empty() {
        ProblemClass::FlatNoTemplates
    } else {
        ProblemClass::FlatTemplates
    }
}

pub fn is_hierarchical(pb: &Problem) -> bool {
    let has_subtask = |ch: &Chronicle| !ch.subtasks.is_empty();

    pb.chronicles
        .iter()
        .map(|instance| &instance.chronicle)
        .any(has_subtask)
        || pb
            .templates
            .iter()
            .map(|templates| &templates.chronicle)
            .any(has_subtask)
}

/// Returns true if the problem provably contains no cycles in the hierarchy.
pub fn hierarchical_is_non_recursive(pb: &Problem) -> bool {
    let model = &pb.context.model;

    // roots of the graphs are all subtasks in concrete chronicles
    let roots = pb
        .chronicles
        .iter()
        .filter(|ch| !model.entails(!ch.chronicle.presence))
        .flat_map(|ch| ch.chronicle.subtasks.iter())
        .map(|subtask| subtask.task_name.as_slice());

    // two task are considered equivalent for the purpose of cycle detection if they are unifiable
    let equiv = |a: &[Atom], b: &[Atom]| model.unifiable_seq(a, b);

    is_acyclic(
        roots,
        // successors of a task are all subtasks of a template chronicle that can refine the tasl.
        |task: &[Atom]| {
            pb.templates
                .iter()
                .filter(move |tl| tl.chronicle.task.iter().any(|t| equiv(task, t)))
                .flat_map(|tl| tl.chronicle.subtasks.iter())
                .map(|st| st.task_name.as_slice())
        },
        equiv,
    )
}

/// Returns true if the graph contains a cycle.
///
/// # Parameters
///
/// - `roots`: entry points to the graph
/// - `succs`: function that assoicates each node with a list of its children
/// - `equiv`: function to test whether a given node was already
fn is_acyclic<T: Sized + Copy, Ts: IntoIterator<Item = T>>(
    roots: impl IntoIterator<Item = T>,
    succs: impl Fn(T) -> Ts,
    equiv: impl Fn(T, T) -> bool,
) -> bool {
    // stack of the depth first search.
    // Each node is labeled with its depth to allow maintaining the path from the root
    let mut stack = Vec::with_capacity(32);
    for x in roots.into_iter() {
        stack.push((x, 0));
    }

    // history of traversed from the root to the current one
    let mut path: Vec<T> = Vec::with_capacity(32);

    // traverse the graph depth first until we exhaust it or en
    while let Some((top, parent_depth)) = stack.pop() {
        path.truncate(parent_depth);
        if path.iter().any(|prev| equiv(*prev, top)) {
            return false;
        }
        for succ in succs(top) {
            stack.push((succ, parent_depth + 1));
        }
        path.push(top);
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_acyclic() {
        assert!(is_acyclic(vec![0, 1], |i| (i + 1)..5, |x, y| x == y));
        assert!(!is_acyclic(vec![0, 1], |i| [(i + 1) % 5], |x, y| x == y));
    }
}
