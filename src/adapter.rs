use std::rc::Rc;

use anyhow::bail;
use anyhow::ensure;
use flatzinc::ConstraintItem;
use flatzinc::Expr;

use crate::constraint::builtins::BoolAnd;
use crate::model::Model;
use crate::var::VarBool;

pub fn bool_and_from_item(item: ConstraintItem, model: &Model) -> anyhow::Result<BoolAnd> {
    ensure!(item.id.as_str() == BoolAnd::NAME, "'bool_and' expected but received '{}'", item.id);
    ensure!(item.exprs.len() == 2, "2 args expected but received {}", item.exprs.len());
    let [a,b] = <[_;2]>::try_from(item.exprs).unwrap();
    let a = var_bool_from_expr(a, model)?;
    let b = var_bool_from_expr(b, model)?;
    Ok(BoolAnd::new(a, b))
}


pub fn var_bool_from_expr(expr: Expr, model: &Model) -> anyhow::Result<Rc<VarBool>> {
    match expr {
        Expr::VarParIdentifier(id) => model.get_var_bool(&id),
        _ => bail!("not a varbool"),
    }
}