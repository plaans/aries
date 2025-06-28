use crate::core::VarRef;
use crate::model::Label;
use crate::model::Model;
use crate::solver::search::beta::var_order::VarOrder;

#[derive(Clone, Debug)]
pub struct FirstFail;

impl<Lbl: Label> VarOrder<Lbl> for FirstFail {
    fn select(&self, model: &Model<Lbl>) -> Option<VarRef> {
        model
            .state
            .variables()
            .filter(|v| !model.state.is_bound(*v))
            .min_by_key(|v| {
                let (lb, ub) = model.state.bounds(*v);
                ub - lb
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn select() {
        let mut model = Model::<String>::new();
        let _ = model.new_ivar(1, 0, "x"); // Empty domain
        let _ = model.new_ivar(3, 5, "y");
        let z = model.new_ivar(0, 1, "z").into();
        assert_eq!(FirstFail.select(&model), Some(z));
    }
}
