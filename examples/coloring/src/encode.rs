use aries::core::state::Cause;
use aries::model::Model;
use aries::model::lang::IVar;
use aries::model::lang::expr::{eq, leq, neq};

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

pub struct Encoding {
    pub n_colors: IVar,
    #[allow(unused)]
    pub nodes: Vec<EncodedNode>,
}

impl Encoding {
    pub fn new(problem: &Problem, model: &mut Model<String>) -> Self {
        // Color int representation
        let min_col = 1;
        let max_col = problem.nodes.len().try_into().unwrap();

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

        // Mark node_color != neighbors and maybe node_color == non-neighbor

        for (i, n1) in nodes.iter().enumerate() {
            for n2 in &nodes[i + 1..] {
                if problem.edges.contains(&(n1.node, n2.node)) || problem.edges.contains(&(n2.node, n1.node)) {
                    let lit = model.reify(neq(n1.color, n2.color));
                    model.state.set(lit, Cause::Encoding).unwrap();
                } else {
                    model.reify(eq(n1.color, n2.color));
                }
            }
        }

        Encoding { n_colors, nodes }
    }
}
