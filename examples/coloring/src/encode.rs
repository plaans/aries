use aries::core::IntCst;
use aries::core::state::Cause;
use aries::model::Model;
use aries::model::lang::IVar;
use aries::model::lang::expr::{eq, leq, neq};

use super::REIFY_EQ;
use crate::parse::{Node, Problem};

pub struct EncodedNode {
    node: Node,
    color: IVar,
}

impl EncodedNode {
    pub fn new(node: Node, color: IVar) -> Self {
        Self { node, color }
    }
}

/// An encoding for a graph coloring problem.
pub struct Encoding {
    /// The chromatic number of the graph, variable to minimize.
    pub n_colors: IVar,

    /// A vec of nodes and their color
    #[allow(unused)]
    pub nodes: Vec<EncodedNode>,
}

impl Encoding {
    /// Create a new encoding for a coloring problem
    pub fn new(problem: &Problem, model: &mut Model<String>) -> Self {
        // Color int representation
        let min_col = 1;
        let max_col = problem.upper_bound() as IntCst;

        // Total number of colros to minimize
        let n_colors = model.new_ivar(min_col, max_col, "n_colors");

        // Mark node color <= n_colors
        let mut nodes = vec![];
        for node in problem.nodes.clone() {
            let node_color = model.new_ivar(min_col, max_col, format!("Node {} col", usize::from(node)));
            let lit = model.reify(leq(node_color, n_colors));
            model.state.set(lit, Cause::Encoding).unwrap();
            nodes.push(EncodedNode::new(node, node_color))
        }

        // Mark node_color != neighbors and node_color == non-neighbor
        for (i, n1) in nodes.iter().enumerate() {
            for n2 in &nodes[i + 1..] {
                if problem.edges.contains(&(n1.node, n2.node)) || problem.edges.contains(&(n2.node, n1.node)) {
                    model.enforce(neq(n1.color, n2.color), []);
                } else if REIFY_EQ.get() {
                    model.reify(eq(n1.color, n2.color));
                } else {
                    model.half_reify(eq(n1.color, n2.color));
                }
            }
        }

        Encoding { n_colors, nodes }
    }
}
