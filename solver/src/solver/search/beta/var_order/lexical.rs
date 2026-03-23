use crate::core::VarRef;
use crate::model::Label;
use crate::model::Model;
use crate::solver::search::beta::var_order::VarOrder;

#[derive(Clone, Debug)]
pub struct Lexical;

impl<Lbl: Label> VarOrder<Lbl> for Lexical {
    fn select(&self, model: &Model<Lbl>) -> Option<VarRef> {
        model.state.variables().find(|v| !model.state.is_bound(*v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select() {
        let mut model = Model::<String>::new();
        let _ = model.new_ivar(1, 0, "x"); // Empty domain
        let y = model.new_ivar(3, 5, "y").into();
        let _ = model.new_ivar(0, 1, "z");
        assert_eq!(Lexical.select(&model), Some(y));
    }
}
