use std::rc::Rc;

use crate::types::Int;

pub trait Flatzinc {
    fn fzn(&self) -> String;
}

impl Flatzinc for bool {
    fn fzn(&self) -> String {
        format!("{}", self)
    }
}

impl Flatzinc for Int {
    fn fzn(&self) -> String {
        format!("{}", self)
    }
}

impl<T: Flatzinc> Flatzinc for Vec<T> {
    fn fzn(&self) -> String {
        format!(
            "[{}]",
            self.iter()
                .map(|x| x.fzn())
                .collect::<Vec<String>>()
                .join(", "),
        )
    }
}

impl<T: Flatzinc> Flatzinc for Rc<T> {
    fn fzn(&self) -> String {
        self.as_ref().fzn()
    }
}
