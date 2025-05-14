use crate::core::VarRef;
use crate::model::Label;
use crate::model::Model;
use crate::solver::search::beta::var_order::VarOrder;

#[derive(Clone, Debug)]
pub struct Lexical;

impl VarOrder for Lexical {
    fn select<Lbl: Label>(&self, model: &Model<Lbl>) -> Option<VarRef> {
        model.state.variables().filter(|v| !model.state.is_bound(*v)).next()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select() {
        let mut model = Model::<String>::new();
        model.new_ivar(1, 0, "x"); // Empty domain
        let y = model.new_ivar(3, 5, "y").into();
        model.new_ivar(0, 1, "z");
        assert_eq!(Lexical.select(&model), Some(y));
    }
}
