use std::{
    collections::HashSet,
    fs::File,
    io::{self, BufRead},
    num::ParseIntError,
    path::Path,
};

fn read_lines(filename: &Path) -> io::Result<io::Lines<io::BufReader<File>>> {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

#[derive(Default)]
pub struct Problem {
    pub edges: HashSet<(Node, Node)>,
    pub nodes: HashSet<Node>,
    pub solution: Option<u32>,
}

impl Problem {
    fn add_edge(&mut self, node1: Node, node2: Node) {
        self.nodes.insert(node1);
        self.nodes.insert(node2);
        self.edges.insert((node1, node2));
    }
    pub fn from_file(path: &Path) -> Self {
        let mut res: Problem = Default::default();
        assert!(path.is_file());
        let lines = read_lines(path).unwrap();
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
