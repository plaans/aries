use crate::chronicles::analysis::TemplateID;
use crate::chronicles::{EffectOp, Fluent, Problem};
use aries::collections::ref_store::IterableRefMap;
use aries::model::symbols::SymId;
use itertools::Itertools;
use std::collections::{HashMap, HashSet};

/// Implementation of "Automatically Generating Abstractions for Planning" by Craig A. Knoblock
/// This one associates each action to an abstraction level which is returned as a map.
/// THe level of the action is the one of the most abstract fluent the action contributes to (i.e. has an effect on).
///
/// Note: 0 is the most abstract level.
pub fn hierarchy(pb: &Problem) -> HashMap<TemplateID, usize> {
    let resource_fluents = resource_fluents(pb);

    let mut links: IterableRefMap<SymId, HashSet<SymId>> = Default::default();

    let mut add_link = |src: SymId, tgt: SymId| {
        if resource_fluents.contains(&src) || resource_fluents.contains(&tgt) {
            // ignore resource fluents in the analysis
            return;
        }
        // for src in pb.context.model.sym_domain_of(src) {
        if !links.contains(src) {
            links.insert(src, HashSet::with_capacity(4));
        }
        // for tgt in pb.context.model.sym_domain_of(tgt) {
        links[src].insert(tgt);
        // }
        // }
    };

    for ch in &pb.templates {
        for eff in &ch.chronicle.effects {
            let eff_fluent = eff.state_var.fluent.sym;

            for cond in &ch.chronicle.conditions {
                let cond_fluent = cond.state_var.fluent.sym;
                add_link(cond_fluent, eff_fluent)
            }

            for eff2 in &ch.chronicle.effects {
                let eff2_fluent = eff2.state_var.fluent.sym;
                add_link(eff_fluent, eff2_fluent)
            }
        }
    }

    let mut scc = tarjan::ordered_scc(&links);
    scc.reverse();
    println!("\nFluent hierarchy:");
    let mut templates_lvl: HashMap<usize, usize> = Default::default();
    for (lvl, group) in scc.iter().enumerate() {
        print!(" - ");
        for fluent_sym in group {
            let sym = pb.context.model.shape.symbols.symbol(*fluent_sym);
            print!("{sym}   ");
            for (template_id, template) in pb.templates.iter().enumerate() {
                if templates_lvl.contains_key(&template_id) {
                    continue;
                }
                for e in &template.chronicle.effects {
                    if &e.state_var.fluent.sym == fluent_sym {
                        templates_lvl.insert(template_id, lvl);
                        break;
                    }
                }
            }
        }
        println!()
    }
    for template_id in 0..pb.templates.len() {
        // templates that have no effect on a fluent in the hierarchy (no attributed level yet)
        // are placed at the last level
        templates_lvl.entry(template_id).or_insert(scc.len());
    }
    println!("\nAction hierarchy: ");

    for (template, lvl) in templates_lvl.iter().sorted_by_key(|(_template, lvl)| **lvl) {
        println!("  [{lvl}] {}", pb.templates[*template].label)
    }

    templates_lvl
}

fn resource_fluents(pb: &Problem) -> HashSet<SymId> {
    pb.context
        .fluents
        .iter()
        .filter_map(|f| if is_resource_fluent(f, pb) { Some(f.sym) } else { None })
        .collect()
}

fn is_resource_fluent(fluent: &Fluent, pb: &Problem) -> bool {
    for ch in &pb.templates {
        for e in &ch.chronicle.effects {
            if e.state_var.fluent.as_ref() == fluent {
                let EffectOp::Assign(val) = e.operation else {
                    return false;
                };
                let has_matching_cond = ch
                    .chronicle
                    .conditions
                    .iter()
                    .any(|c| c.state_var == e.state_var && c.value == val && c.end == e.transition_start);
                if !has_matching_cond {
                    return false;
                }
            }
        }
    }
    true
}

mod tarjan {
    pub fn ordered_scc(graph: &Graph) -> Vec<Vec<SymId>> {
        let scc = StronglyConnectedComponents::new(graph);
        let mut components = vec![vec![]; scc.num_components];
        for (vertex, component_id) in scc.component.entries() {
            components[scc.num_components - *component_id].push(vertex);
        }
        components
    }

    // Adapted from https://github.com/TheAlgorithms/Rust/blob/master/src/graph/strongly_connected_components.rs

    /*
    Tarjan's algorithm to find Strongly Connected Components (SCCs):
    It runs in O(n + m) (so it is optimal) and as a by-product, it returns the
    components in some (reverse) topologically sorted order.

    We assume that graph is represented using (compressed) adjacency matrix
    and its vertices are numbered from 1 to n. If this is not the case, one
    can use `src/graph/graph_enumeration.rs` to convert their graph.
    */

    use aries::collections::ref_store::IterableRefMap;
    use aries::model::symbols::SymId;
    use std::collections::HashSet;

    type V = SymId;
    type Graph = IterableRefMap<SymId, HashSet<SymId>>;

    pub struct StronglyConnectedComponents {
        // The number of the SCC the vertex is in, starting from 1
        pub component: IterableRefMap<V, usize>,

        // The discover time of the vertex with minimum discover time reachable
        // from this vertex. The MSB of the numbers are used to save whether the
        // vertex has been visited (but the MSBs are cleared after
        // the algorithm is done)
        pub state: IterableRefMap<V, u64>,

        // The total number of SCCs
        pub num_components: usize,

        // The stack of vertices that DFS has seen (used internally)
        stack: Vec<V>,
        // Used internally during DFS to know the current discover time
        current_time: usize,
    }

    // Some functions to help with DRY and code readability
    const NOT_DONE: u64 = 1 << 63;

    #[inline]
    fn set_done(vertex_state: &mut u64) {
        *vertex_state ^= NOT_DONE;
    }

    #[inline]
    fn is_in_stack(vertex_state: u64) -> bool {
        vertex_state != 0 && (vertex_state & NOT_DONE) != 0
    }

    #[inline]
    fn is_unvisited(vertex_state: u64) -> bool {
        vertex_state == NOT_DONE
    }

    #[inline]
    fn get_discover_time(vertex_state: u64) -> u64 {
        vertex_state ^ NOT_DONE
    }

    impl StronglyConnectedComponents {
        pub fn new(graph: &Graph) -> Self {
            let mut scc = StronglyConnectedComponents {
                component: Default::default(),
                state: Default::default(),
                num_components: 0,
                stack: vec![],
                current_time: 1,
            };
            for vertex in graph.keys() {
                scc.component.insert(vertex, 0);
                scc.state.insert(vertex, NOT_DONE);
            }

            for v in graph.keys() {
                if is_unvisited(scc.state[v]) {
                    scc.dfs(v, graph);
                }
            }
            scc
        }
        fn dfs(&mut self, v: V, adj: &Graph) -> u64 {
            let mut min_disc = self.current_time as u64;
            // self.state[v] = NOT_DONE + min_disc
            self.state[v] ^= min_disc;
            self.current_time += 1;
            self.stack.push(v);

            for &u in adj[v].iter() {
                if is_unvisited(self.state[u]) {
                    min_disc = std::cmp::min(self.dfs(u, adj), min_disc);
                } else if is_in_stack(self.state[u]) {
                    min_disc = std::cmp::min(get_discover_time(self.state[u]), min_disc);
                }
            }

            // No vertex with a lower discovery time is reachable from this one
            // So it should be "the head" of a new SCC.
            if min_disc == get_discover_time(self.state[v]) {
                self.num_components += 1;
                loop {
                    let u = self.stack.pop().unwrap();
                    self.component[u] = self.num_components;
                    set_done(&mut self.state[u]);
                    if u == v {
                        break;
                    }
                }
            }

            min_disc
        }
    }
}
