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
    pub fn parse(input: &str) -> Result<CNF, String> {
        let mut cnf = CNF::new();
        let mut lines_iter = input.lines().filter(|l| !l.starts_with('c'));
        let header = lines_iter.next();
        if header.and_then(|h| h.chars().next()) != Some('p') {
            return Err("No header line starting with 'p'".to_string());
        }
        let mut lits = Vec::with_capacity(32);
        for l in lines_iter {
            lits.clear();
            for lit in l.split_whitespace() {
                match lit.parse::<i32>() {
                    Ok(0) => break,
                    Ok(i) => lits.push(Lit::from_signed_int(i).unwrap()),
                    Err(_) => return Err(format!("Invalid literal: {}", lit)),
                }
            }
            cnf.add_clause(&lits);
        }
        Ok(cnf)
    }
}
