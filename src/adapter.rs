use std::rc::Rc;

use anyhow::bail;
use anyhow::ensure;
use flatzinc::ConstraintItem;
use flatzinc::Expr;
use flatzinc::OptimizationType;

use crate::constraint::builtins::BoolAnd;
use crate::constraint::builtins::IntEq;
use crate::model::Model;
use crate::solve::Goal;
use crate::var::VarBool;
use crate::var::VarInt;

pub fn bool_and_from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<BoolAnd> {
    ensure!(item.id.as_str() == BoolAnd::NAME, "'{}' expected but received '{}'", BoolAnd::NAME, item.id);
    ensure!(item.exprs.len() == 2, "2 args expected but received {}", item.exprs.len());
    let [a,b] = <[_;2]>::try_from(item.exprs).unwrap();
    let a = var_bool_from_expr(a, model)?;
    let b = var_bool_from_expr(b, model)?;
    Ok(BoolAnd::new(a, b))
}

pub fn int_eq_from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<IntEq> {
    ensure!(item.id.as_str() == IntEq::NAME, "'{}' expected but received '{}'", IntEq::NAME, item.id);
    ensure!(item.exprs.len() == 2, "2 args expected but received {}", item.exprs.len());
    let [a,b] = <[_;2]>::try_from(item.exprs).unwrap();
    let a = var_int_from_expr(a, model)?;
    let b = var_int_from_expr(b, model)?;
    Ok(IntEq::new(a, b))
}


pub fn var_bool_from_expr(expr: Expr, model: &Model) -> anyhow::Result<Rc<VarBool>> {
    match expr {
        Expr::VarParIdentifier(id) => model.get_var_bool(&id),
        _ => bail!("not a varbool"),
    }
}

pub fn var_int_from_expr(expr: Expr, model: &Model) -> anyhow::Result<Rc<VarInt>> {
    match expr {
        Expr::VarParIdentifier(id) => model.get_var_int(&id),
        _ => bail!("not a varbool"),
    }
}

pub fn convert_goal(optim: OptimizationType) -> Goal {
    match optim {
        OptimizationType::Minimize => Goal::Minimize,
        OptimizationType::Maximize => Goal::Maximize,
    }
}