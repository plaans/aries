use std::{
    collections::{HashMap, HashSet},
    fs::File,
    io::{self, BufRead},
    num::ParseIntError,
    path::Path,
};

fn read_lines(filename: &Path) -> io::Result<io::Lines<io::BufReader<File>>> {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

/// A graph represented as a set of nodes and a set of edges.
/// May optionally be have a know chromatic number.
#[derive(Default)]
pub struct Problem {
    pub edges: HashSet<(Node, Node)>,
    pub nodes: HashSet<Node>,
    pub solution: Option<u32>,
}

impl Problem {
    /// Get an upper bound on the number of colors.
    pub fn upper_bound(&self) -> u32 {
        let mut n_edges = HashMap::new();
        for (source, target) in self.edges.iter() {
            n_edges.entry(source).and_modify(|x| *x += 1).or_insert(1);
            n_edges.entry(target).and_modify(|x| *x += 1).or_insert(1);
        }
        n_edges.into_values().max().unwrap() + 1
    }

    fn add_edge(&mut self, node1: Node, node2: Node) {
        assert!(!self.edges.contains(&(node2, node1)));
        self.nodes.insert(node1);
        self.nodes.insert(node2);
        self.edges.insert((node1, node2));
    }

    /// Load a problem from a .col file
    ///
    /// The file must contain a newline seperated list of edges, e.g.:
    /// e 0 1
    /// e 1 2
    ///
    /// Everything else is ignored
    pub fn from_file(path: &Path) -> Self {
        let mut res: Problem = Default::default();
        assert!(path.is_file());
        let lines = read_lines(path).expect("File provided was not able to be read.");
        for line in lines.map_while(Result::ok) {
            if line.starts_with("e") {
                let mut split = line.split_whitespace();
                split.next().unwrap();
                let node1 = split.next().unwrap().try_into().unwrap();
                let node2 = split.next().unwrap().try_into().unwrap();
                res.add_edge(node1, node2);
            }
        }
        res
    }

    #[allow(unused)]
    pub fn check_solution(&self, proposed_solution: u32) {
        if let Some(solution) = self.solution {
            assert_eq!(solution, proposed_solution)
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub struct Node(usize);

impl From<Node> for usize {
    fn from(value: Node) -> Self {
        value.0
    }
}

impl From<usize> for Node {
    fn from(value: usize) -> Self {
        Node(value)
    }
}

impl TryFrom<&str> for Node {
    type Error = ParseIntError;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        value.parse::<usize>().map(|u| u.into())
    }
}
