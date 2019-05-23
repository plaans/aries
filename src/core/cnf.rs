use crate::core::all::Lit;
use crate::core::clause::Clause;

pub struct CNF {
    pub num_vars: u32,
    pub clauses: Vec<Clause>,
}

impl CNF {
    pub fn new() -> Self {
        CNF {
            num_vars: 0,
            clauses: Vec::new(),
        }
    }

    pub fn add_clause(&mut self, lits: Clause) {
        lits.disjuncts.iter().for_each(|l| {
            self.num_vars = self.num_vars.max(l.variable().id.get());
        });
        self.clauses.push(lits);
    }

    pub fn parse(input: &str) -> CNF {
        let mut cnf = CNF::new();
        let mut lines_iter = input.lines().filter(|l| l.chars().next() != Some('c'));
        let header = lines_iter.next();
        println!("{:?}", header);
        assert!(header.and_then(|h| h.chars().next()) == Some('p'));
        for l in lines_iter {
            let mut lits = vec![];
            l.split_whitespace()
                .map(|lit| lit.parse::<i32>().unwrap())
                .take_while(|i| *i != 0)
                .for_each(|l| lits.push(Lit::from_signed_int(l)));
            let cl = Clause::new(&lits[..]);
            cnf.add_clause(cl);
        }
        cnf
    }
}
