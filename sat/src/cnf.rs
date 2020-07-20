use crate::all::Lit;

#[derive(Default)]
pub struct CNF {
    pub num_vars: u32,
    pub clauses: Vec<Box<[Lit]>>,
}

impl CNF {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn add_clause(&mut self, lits: &[Lit]) {
        lits.iter().for_each(|l| {
            self.num_vars = self.num_vars.max(l.variable().id.get());
        });
        self.clauses.push(lits.to_vec().into_boxed_slice());
    }

    /// Parses a set of clauses in CNF format (see `problems/cnf` for example)
    /// TODO: make robust to input error
    pub fn parse(input: &str) -> CNF {
        let mut cnf = CNF::new();
        let mut lines_iter = input.lines().filter(|l| !l.starts_with('c'));
        let header = lines_iter.next();
        assert_eq!(header.and_then(|h| h.chars().next()), Some('p'));
        for l in lines_iter {
            let lits = l
                .split_whitespace()
                .map(|lit| lit.parse::<i32>().unwrap())
                .take_while(|i| *i != 0)
                .map(Lit::from_signed_int)
                .collect::<Vec<_>>();

            cnf.add_clause(&lits[..]);
        }
        cnf
    }
}
