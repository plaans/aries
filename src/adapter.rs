use std::rc::Rc;

use anyhow::bail;
use flatzinc::BoolExpr;
use flatzinc::Expr;
use flatzinc::OptimizationType;

use crate::model::Model;
use crate::solve::Goal;
use crate::types::Int;
use crate::var::VarBool;
use crate::var::VarInt;

pub fn goal_from_optim_type(optim: &OptimizationType) -> Goal {
    match optim {
        OptimizationType::Minimize => Goal::Minimize,
        OptimizationType::Maximize => Goal::Maximize,
    }
}

pub fn var_bool_from_expr(expr: &Expr, model: &Model) -> anyhow::Result<Rc<VarBool>> {
    match expr {
        Expr::VarParIdentifier(id) => model.get_var_bool(&id),
        _ => bail!("not a varbool"),
    }
}

pub fn var_int_from_expr(expr: &Expr, model: &Model) -> anyhow::Result<Rc<VarInt>> {
    match expr {
        Expr::VarParIdentifier(id) => model.get_var_int(&id),
        _ => bail!("not a varint"),
    }
}

pub fn bool_from_expr(expr: &Expr, model: &Model) -> anyhow::Result<bool> {
    match expr {
        Expr::VarParIdentifier(id) => Ok(*model.get_par_bool(id)?.value()),
        _ => bail!("not a varbool"),
    }
}

pub fn int_from_expr(expr: &Expr, model: &Model) -> anyhow::Result<Int> {
    match expr {
        Expr::VarParIdentifier(id) => Ok(*model.get_par_int(id)?.value()),
        Expr::Int(x) => Ok(*x as Int),
        _ => bail!("not an int"),
    }
}

pub fn var_bool_vec_from_expr(expr: &Expr, model: &Model) -> anyhow::Result<Vec<Rc<VarBool>>> {
    match expr {
        Expr::VarParIdentifier(id) => Ok(
            model.get_var_bool_array(id)?
                .variables()
                .cloned()
                .collect()
        ),
        Expr::ArrayOfBool(v) => v
            .iter()
            .map(|e| var_bool_from_bool_expr(e, model))
            .collect(),
        _ => todo!(),
    }
}

pub fn var_bool_from_bool_expr(expr: &BoolExpr, model: &Model) -> anyhow::Result<Rc<VarBool>> {
    match expr {
        BoolExpr::VarParIdentifier(id) => model.get_var_bool(&id),
        BoolExpr::Bool(_) => bail!("unexpected bool literal"),
    }
}
