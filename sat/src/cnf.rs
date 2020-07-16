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
}
