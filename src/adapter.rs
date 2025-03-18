use std::rc::Rc;

use anyhow::bail;
use flatzinc::BoolExpr;
use flatzinc::Expr;
use flatzinc::IntExpr;
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

pub fn var_bool_from_expr(
    expr: &Expr,
    model: &Model,
) -> anyhow::Result<Rc<VarBool>> {
    match expr {
        Expr::VarParIdentifier(id) => model.get_var_bool(&id),
        _ => bail!("not a varbool"),
    }
}

pub fn var_int_from_expr(
    expr: &Expr,
    model: &Model,
) -> anyhow::Result<Rc<VarInt>> {
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

pub fn bool_from_bool_expr(
    expr: &BoolExpr,
    model: &Model,
) -> anyhow::Result<bool> {
    match expr {
        BoolExpr::Bool(b) => Ok(*b),
        BoolExpr::VarParIdentifier(id) => Ok(*model.get_par_bool(id)?.value()),
    }
}

pub fn int_from_int_expr(expr: &IntExpr, model: &Model) -> anyhow::Result<Int> {
    match expr {
        IntExpr::Int(x) => Ok(*x as Int),
        IntExpr::VarParIdentifier(id) => Ok(*model.get_par_int(id)?.value()),
    }
}

pub fn int_from_expr(expr: &Expr, model: &Model) -> anyhow::Result<Int> {
    match expr {
        Expr::VarParIdentifier(id) => Ok(*model.get_par_int(id)?.value()),
        Expr::Int(x) => Ok(*x as Int),
        _ => bail!("not an int"),
    }
}

pub fn vec_var_bool_from_expr(
    expr: &Expr,
    model: &Model,
) -> anyhow::Result<Vec<Rc<VarBool>>> {
    match expr {
        Expr::VarParIdentifier(id) => {
            Ok(model.get_var_bool_array(id)?.variables().cloned().collect())
        }
        Expr::ArrayOfBool(v) => v
            .iter()
            .map(|e| var_bool_from_bool_expr(e, model))
            .collect(),
        _ => todo!(),
    }
}

pub fn var_bool_from_bool_expr(
    expr: &BoolExpr,
    model: &Model,
) -> anyhow::Result<Rc<VarBool>> {
    match expr {
        BoolExpr::VarParIdentifier(id) => model.get_var_bool(&id),
        BoolExpr::Bool(_) => bail!("unexpected bool literal"),
    }
}

pub fn var_int_from_int_expr(
    expr: &IntExpr,
    model: &Model,
) -> anyhow::Result<Rc<VarInt>> {
    match expr {
        IntExpr::VarParIdentifier(id) => model.get_var_int(&id),
        IntExpr::Int(_) => bail!("unexpected int literal"),
    }
}

// Array of identifiers might be detected as bool array
pub fn var_int_from_bool_expr(
    expr: &BoolExpr,
    model: &Model,
) -> anyhow::Result<Rc<VarInt>> {
    match expr {
        BoolExpr::VarParIdentifier(id) => model.get_var_int(&id),
        BoolExpr::Bool(_) => bail!("unexpected bool literal"),
    }
}

pub fn vec_int_from_expr(
    expr: &Expr,
    model: &Model,
) -> anyhow::Result<Vec<Int>> {
    match expr {
        Expr::ArrayOfInt(int_exprs) => int_exprs
            .iter()
            .map(|e| int_from_int_expr(e, model))
            .collect(),
        Expr::VarParIdentifier(_) => {
            todo!("if it is a par int array, it should be ok")
        }
        _ => bail!("not an int vec"),
    }
}

pub fn vec_var_int_from_expr(
    expr: &Expr,
    model: &Model,
) -> anyhow::Result<Vec<Rc<VarInt>>> {
    match expr {
        Expr::VarParIdentifier(_) => {
            todo!("if it is a var int array, it should be ok")
        }
        Expr::ArrayOfInt(int_exprs) => int_exprs
            .iter()
            .map(|e| var_int_from_int_expr(e, model))
            .collect(),
        // Array of identifier might be detected as array of bool
        Expr::ArrayOfBool(bool_exprs) => bool_exprs
            .iter()
            .map(|e| var_int_from_bool_expr(e, model))
            .collect(),
        _ => bail!("not a vec var int"),
    }
}
