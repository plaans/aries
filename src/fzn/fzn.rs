use std::rc::Rc;

use crate::fzn::types::Int;

pub trait Fzn {
    fn fzn(&self) -> String;
}

impl Fzn for bool {
    fn fzn(&self) -> String {
        format!("{}", self)
    }
}

impl Fzn for Int {
    fn fzn(&self) -> String {
        format!("{}", self)
    }
}

impl<T: Fzn> Fzn for Vec<T> {
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

impl<T: Fzn> Fzn for Rc<T> {
    fn fzn(&self) -> String {
        self.as_ref().fzn()
    }
}
