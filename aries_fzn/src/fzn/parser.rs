//! Flatzinc parsing.

use std::rc::Rc;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use flatzinc::Annotation;
use flatzinc::ArrayOfBoolExpr;
use flatzinc::ArrayOfIntExpr;
use flatzinc::ConstraintItem;
use flatzinc::Expr;
use flatzinc::OptimizationType;
use flatzinc::ParDeclItem;
use flatzinc::Stmt;
use flatzinc::VarDeclItem;

use crate::fzn::constraint::builtins::*;
use crate::fzn::domain::BoolDomain;
use crate::fzn::domain::IntDomain;
use crate::fzn::domain::IntRange;
use crate::fzn::domain::IntSet;
use crate::fzn::model::Model;
use crate::fzn::solve::Goal;
use crate::fzn::types::Int;
use crate::fzn::var::BasicVar;
use crate::fzn::var::VarBool;
use crate::fzn::var::VarInt;

/// Convert a flatzinc [OptimizationType] into a [Goal].
pub fn goal_from_optim_type(optim: &OptimizationType) -> Goal {
    match optim {
        OptimizationType::Minimize => Goal::Minimize,
        OptimizationType::Maximize => Goal::Maximize,
    }
}

/// Return `true` iff the annotation asks for output.
///
/// Remark: it only check the annotation id.
pub fn is_output_anno(anno: &Annotation) -> bool {
    ["output_var", "output_array"].contains(&anno.id.as_str())
}

/// Convert a flatzinc [Expr] into a [VarBool].
pub fn var_bool_from_expr(
    expr: &Expr,
    model: &mut Model,
) -> anyhow::Result<Rc<VarBool>> {
    match expr {
        Expr::VarParIdentifier(id) => model.get_var_bool(id),
        Expr::Bool(x) => Ok(model.new_var_bool_const((*x).try_into()?)),
        _ => bail!("not a varbool"),
    }
}

/// Convert a flatzinc [Expr] into a [VarInt].
pub fn var_int_from_expr(
    expr: &Expr,
    model: &mut Model,
) -> anyhow::Result<Rc<VarInt>> {
    match expr {
        Expr::VarParIdentifier(id) => {
            if let Ok(var) = model.get_var_int(id) {
                return Ok(var);
            }
            if let Ok(par) = model.get_par_int(id) {
                return Ok(model.new_var_int_const(*par.value()));
            }
            bail!(format!("no varint named '{}'", id))
        }
        Expr::Int(x) => Ok(model.new_var_int_const((*x).try_into()?)),
        _ => bail!("not a varint"),
    }
}

/// Convert a flatzinc [Expr] into a [BasicVar].
pub fn basic_var_from_expr(
    expr: &Expr,
    model: &mut Model,
) -> anyhow::Result<BasicVar> {
    if let Ok(var) = var_int_from_expr(expr, model) {
        return Ok(var.into());
    }
    if let Ok(var) = var_bool_from_expr(expr, model) {
        return Ok(var.into());
    }
    bail!("not a basic var")
}

/// Convert a flatzinc [Expr] into a boolean.
pub fn bool_from_expr(expr: &Expr, model: &Model) -> anyhow::Result<bool> {
    match expr {
        Expr::VarParIdentifier(id) => Ok(*model.get_par_bool(id)?.value()),
        Expr::Bool(b) => Ok(*b),
        _ => bail!("not a bool"),
    }
}

/// Convert a flatzinc [Expr] into an [Int].
pub fn int_from_expr(expr: &Expr, model: &Model) -> anyhow::Result<Int> {
    match expr {
        Expr::VarParIdentifier(id) => Ok(*model.get_par_int(id)?.value()),
        Expr::Int(x) => Ok(*x as Int),
        _ => bail!("not an int"),
    }
}

/// Convert a flatzinc [Expr] into a vector of [VarBool].
pub fn vec_var_bool_from_expr(
    expr: &Expr,
    model: &mut Model,
) -> anyhow::Result<Vec<Rc<VarBool>>> {
    match expr {
        Expr::VarParIdentifier(id) => {
            Ok(model.get_var_bool_array(id)?.variables().cloned().collect())
        }
        Expr::ArrayOfBool(v) => v
            .iter()
            .cloned()
            .map(|e| var_bool_from_expr(&e.into(), model))
            .collect(),
        _ => todo!(),
    }
}

/// Convert a flatzinc [Expr] into a vector of bool.
pub fn vec_bool_from_expr(
    expr: &Expr,
    model: &Model,
) -> anyhow::Result<Vec<bool>> {
    match expr {
        Expr::ArrayOfBool(bool_exprs) => bool_exprs
            .iter()
            .cloned()
            .map(|e| bool_from_expr(&e.into(), model))
            .collect(),
        Expr::VarParIdentifier(id) => {
            model.get_par_bool_array(id).map(|p| p.value().clone())
        }
        // Array might be detected as array of int
        Expr::ArrayOfInt(int_exprs) => int_exprs
            .iter()
            .cloned()
            .map(|e| bool_from_expr(&e.into(), model))
            .collect(),
        _ => bail!("not a bool vec"),
    }
}

/// Convert a flatzinc [Expr] into a vector of [Int].
pub fn vec_int_from_expr(
    expr: &Expr,
    model: &Model,
) -> anyhow::Result<Vec<Int>> {
    match expr {
        Expr::ArrayOfInt(int_exprs) => int_exprs
            .iter()
            .cloned()
            .map(|e| int_from_expr(&e.into(), model))
            .collect(),
        Expr::VarParIdentifier(id) => {
            model.get_par_int_array(id).map(|p| p.value().clone())
        }
        // Array might be detected as array of bool
        Expr::ArrayOfBool(bool_exprs) => bool_exprs
            .iter()
            .cloned()
            .map(|e| int_from_expr(&e.into(), model))
            .collect(),
        _ => bail!("not an int vec"),
    }
}

/// Convert a flatzinc [Expr] into a vector of [VarInt].
pub fn vec_var_int_from_expr(
    expr: &Expr,
    model: &mut Model,
) -> anyhow::Result<Vec<Rc<VarInt>>> {
    match expr {
        Expr::VarParIdentifier(id) => {
            Ok(model.get_var_int_array(id)?.variables().cloned().collect())
        }
        Expr::ArrayOfInt(int_exprs) => int_exprs
            .iter()
            .cloned()
            .map(|e| var_int_from_expr(&e.into(), model))
            .collect(),
        // Array of identifier might be detected as array of bool
        Expr::ArrayOfBool(bool_exprs) => bool_exprs
            .iter()
            .cloned()
            .map(|e| var_int_from_expr(&e.into(), model))
            .collect(),
        _ => bail!("not a vec var int"),
    }
}

/// Parse a flatzinc string into a new [Model].
pub fn parse_model(content: &str) -> anyhow::Result<Model> {
    let mut model = Model::new();
    let mut nb_solve_items = 0;

    for (i, line) in content.lines().enumerate() {
        let is_solve_item = parse_line(line, &mut model).context(format!(
            "parsing failure at line {}:\n{}\n",
            i + 1,
            line
        ))?;
        if is_solve_item {
            nb_solve_items += 1;
        }
    }

    anyhow::ensure!(
        nb_solve_items == 1,
        "exactly one solve statement is expected"
    );
    Ok(model)
}

/// Update the model with the given flatzinc line
///
/// Return `true` if the line is a solve item.
pub fn parse_line(line: &str, model: &mut Model) -> anyhow::Result<bool> {
    let statement = flatzinc::Stmt::from_str(line).map_err(|e| anyhow!(e))?;
    let is_solve_item = matches!(statement, Stmt::SolveItem(_));
    match statement {
        Stmt::Comment(_) => {}
        Stmt::Parameter(par_decl_item) => {
            parse_par_decl_item(par_decl_item, model)?
        }
        Stmt::Variable(var_decl_item) => {
            parse_var_decl_item(var_decl_item, model)?
        }
        Stmt::Constraint(constraint_item) => {
            parse_constraint_item(constraint_item, model)?
        }
        Stmt::SolveItem(solve_item) => parse_solve_item(solve_item, model)?,
        Stmt::Predicate(_) => { /* ignore predicate declaration */ }
    }
    Ok(is_solve_item)
}

/// Update the model with the given parameter declaration.
pub fn parse_par_decl_item(
    par_decl_item: ParDeclItem,
    model: &mut Model,
) -> anyhow::Result<()> {
    match par_decl_item {
        ParDeclItem::Bool { id, bool } => {
            model.new_par_bool(id, bool)?;
        }
        ParDeclItem::Int { id, int } => {
            model.new_par_int(id, int.try_into()?)?;
        }
        ParDeclItem::ArrayOfInt { ix: _, id, v } => {
            let value: Vec<Int> = v.iter().map(|x| *x as Int).collect();
            model.new_par_int_array(id, value)?;
        }
        _ => todo!(),
    }
    Ok(())
}

/// Update the model with a variable declaration.
pub fn parse_var_decl_item(
    var_decl_item: VarDeclItem,
    model: &mut Model,
) -> anyhow::Result<()> {
    match var_decl_item {
        VarDeclItem::Bool { id, expr, annos } => {
            let output = annos.iter().any(is_output_anno);
            match expr {
                Some(e) => {
                    let value = bool_from_expr(&e.into(), model)?;
                    model.new_var_bool(
                        BoolDomain::Singleton(value),
                        id,
                        output,
                    )?;
                }
                None => {
                    model.new_var_bool(BoolDomain::Both, id, output)?;
                }
            }
        }
        VarDeclItem::IntInRange {
            id,
            lb,
            ub,
            expr,
            annos,
        } => {
            let output = annos.iter().any(is_output_anno);
            let lb = Int::try_from(lb).unwrap();
            let ub = Int::try_from(ub).unwrap();
            let domain = if let Some(e) = expr {
                let value = int_from_expr(&e.into(), model)?;
                ensure!(
                    lb <= value && value <= ub,
                    "{} is not in {}..{}",
                    value,
                    lb,
                    ub
                );
                IntDomain::Singleton(value)
            } else {
                IntRange::new(lb, ub)?.into()
            };
            model.new_var_int(domain, id, output)?;
        }
        VarDeclItem::ArrayOfBool {
            ix: _,
            id,
            annos,
            array_expr,
        } => {
            let output = annos.iter().any(is_output_anno);
            let e = array_expr.expect("expected array expression");
            match e {
                ArrayOfBoolExpr::Array(bool_exprs) => {
                    let vars: anyhow::Result<Vec<Rc<VarBool>>> = bool_exprs
                        .iter()
                        .cloned()
                        .map(|e| var_bool_from_expr(&e.into(), model))
                        .collect();
                    model.new_var_bool_array(vars?, id, output)?;
                }
                ArrayOfBoolExpr::VarParIdentifier(id) => {
                    let var = model.get_var_bool_array(&id)?;
                    model.new_var_bool_array(
                        var.variables().cloned().collect(),
                        id,
                        output,
                    )?;
                }
            };
        }
        VarDeclItem::ArrayOfInt {
            ix: _,
            id,
            annos,
            array_expr,
        } => {
            let output = annos.iter().any(is_output_anno);
            let e = array_expr.expect("expected array expression");
            match e {
                ArrayOfIntExpr::Array(int_exprs) => {
                    let vars: anyhow::Result<Vec<Rc<VarInt>>> = int_exprs
                        .iter()
                        .cloned()
                        .map(|e| var_int_from_expr(&e.into(), model))
                        .collect();
                    model.new_var_int_array(vars?, id, output)?;
                }
                ArrayOfIntExpr::VarParIdentifier(id) => {
                    let var = model.get_var_int_array(&id)?;
                    model.new_var_int_array(
                        var.variables().cloned().collect(),
                        id,
                        output,
                    )?;
                }
            };
        }
        VarDeclItem::IntInSet {
            id,
            set,
            expr,
            annos,
        } => {
            let output = annos.iter().any(is_output_anno);
            let set = IntSet::from_iter(set.iter().map(|x| *x as Int));
            ensure!(!set.is_empty(), "empty set");
            let domain = if let Some(e) = expr {
                let value = int_from_expr(&e.into(), model)?;
                ensure!(set.contains(&value), "{} is not in the set", value,);
                IntDomain::Singleton(value)
            } else {
                IntDomain::Set(set)
            };
            model.new_var_int(domain, id, output)?;
        }
        VarDeclItem::Int {
            id: _,
            expr: _,
            annos: _,
        } => todo!("unbounded int are not supported"),
        VarDeclItem::Float {
            id: _,
            expr: _,
            annos: _,
        } => bail!("float are not supported"),
        VarDeclItem::BoundedFloat {
            id: _,
            lb: _,
            ub: _,
            expr: _,
            annos: _,
        } => bail!("float int are not supported"),
        VarDeclItem::SetOfInt {
            id: _,
            expr: _,
            annos: _,
        } => bail!("set of int are not supported"),
        VarDeclItem::SubSetOfIntSet {
            id: _,
            set: _,
            expr: _,
            annos: _,
        } => bail!("subset of int set are not supported"),
        VarDeclItem::SubSetOfIntRange {
            id: _,
            lb: _,
            ub: _,
            expr: _,
            annos: _,
        } => bail!("subset of int range are not supported"),
        VarDeclItem::ArrayOfIntInRange {
            lb: _,
            ub: _,
            ix: _,
            id: _,
            annos: _,
            array_expr: _,
        } => todo!("array of int in range are not supported"),
        VarDeclItem::ArrayOfIntInSet {
            set: _,
            ix: _,
            id: _,
            annos: _,
            array_expr: _,
        } => todo!("array of int in set are not supported"),
        VarDeclItem::ArrayOfFloat {
            ix: _,
            id: _,
            annos: _,
            array_expr: _,
        } => bail!("float are not supported"),
        VarDeclItem::ArrayOfBoundedFloat {
            lb: _,
            ub: _,
            ix: _,
            id: _,
            annos: _,
            array_expr: _,
        } => bail!("float are not supported"),
        VarDeclItem::ArrayOfSet {
            ix: _,
            id: _,
            annos: _,
            array_expr: _,
        } => bail!("array of set are not supported"),
        VarDeclItem::ArrayOfSubSetOfIntRange {
            ub: _,
            lb: _,
            ix: _,
            id: _,
            annos: _,
            array_expr: _,
        } => bail!("array of set of int range are not supported"),
        VarDeclItem::ArrayOfSubSetOfIntSet {
            set: _,
            ix: _,
            id: _,
            annos: _,
            array_expr: _,
        } => bail!("array of subset of int set are not supported"),
    }
    Ok(())
}

/// Update the model with the given constraint item.
pub fn parse_constraint_item(
    c: ConstraintItem,
    m: &mut Model,
) -> anyhow::Result<()> {
    let constraint = match c.id.as_str() {
        ArrayBoolAnd::NAME => ArrayBoolAnd::try_from_item(c, m)?.into(),
        ArrayBoolElement::NAME => ArrayBoolElement::try_from_item(c, m)?.into(),
        ArrayIntElement::NAME => ArrayIntElement::try_from_item(c, m)?.into(),
        ArrayIntMaximum::NAME => ArrayIntMaximum::try_from_item(c, m)?.into(),
        ArrayIntMinimum::NAME => ArrayIntMinimum::try_from_item(c, m)?.into(),
        ArrayVarBoolElement::NAME => {
            ArrayVarBoolElement::try_from_item(c, m)?.into()
        }
        ArrayVarIntElement::NAME => {
            ArrayVarIntElement::try_from_item(c, m)?.into()
        }
        Bool2Int::NAME => Bool2Int::try_from_item(c, m)?.into(),
        BoolClause::NAME => BoolClause::try_from_item(c, m)?.into(),
        BoolClauseReif::NAME => BoolClauseReif::try_from_item(c, m)?.into(),
        BoolEq::NAME => BoolEq::try_from_item(c, m)?.into(),
        BoolEqReif::NAME => BoolEqReif::try_from_item(c, m)?.into(),
        BoolLe::NAME => BoolLe::try_from_item(c, m)?.into(),
        BoolLeReif::NAME => BoolLeReif::try_from_item(c, m)?.into(),
        BoolLinEq::NAME => BoolLinEq::try_from_item(c, m)?.into(),
        BoolLinLe::NAME => BoolLinLe::try_from_item(c, m)?.into(),
        BoolNot::NAME => BoolNot::try_from_item(c, m)?.into(),
        BoolXor::NAME => BoolXor::try_from_item(c, m)?.into(),
        IntAbs::NAME => IntAbs::try_from_item(c, m)?.into(),
        IntEq::NAME => IntEq::try_from_item(c, m)?.into(),
        IntEqReif::NAME => IntEqReif::try_from_item(c, m)?.into(),
        IntLe::NAME => IntLe::try_from_item(c, m)?.into(),
        IntLeReif::NAME => IntLeReif::try_from_item(c, m)?.into(),
        IntLt::NAME => IntLt::try_from_item(c, m)?.into(),
        IntLtReif::NAME => IntLtReif::try_from_item(c, m)?.into(),
        IntLinEq::NAME => IntLinEq::try_from_item(c, m)?.into(),
        IntLinEqImp::NAME => IntLinEqImp::try_from_item(c, m)?.into(),
        IntLinEqReif::NAME => IntLinEqReif::try_from_item(c, m)?.into(),
        IntLinLe::NAME => IntLinLe::try_from_item(c, m)?.into(),
        IntLinLeImp::NAME => IntLinLeImp::try_from_item(c, m)?.into(),
        IntLinLeReif::NAME => IntLinLeReif::try_from_item(c, m)?.into(),
        IntLinNe::NAME => IntLinNe::try_from_item(c, m)?.into(),
        IntLinNeReif::NAME => IntLinNeReif::try_from_item(c, m)?.into(),
        IntNe::NAME => IntNe::try_from_item(c, m)?.into(),
        IntNeReif::NAME => IntNeReif::try_from_item(c, m)?.into(),
        _ => anyhow::bail!(format!("unknown constraint '{}'", c.id)),
    };
    m.add_constraint(constraint);
    Ok(())
}

// The optimize items do not rely on the type since flatzinc crate is not able to infer types for identifier
/// Update the model with the given solve item.
pub fn parse_solve_item(
    s_item: flatzinc::SolveItem,
    model: &mut Model,
) -> anyhow::Result<()> {
    match s_item.goal {
        flatzinc::Goal::Satisfy => {}
        flatzinc::Goal::OptimizeBool(optim, expr) => {
            let goal = goal_from_optim_type(&optim);
            let variable = basic_var_from_expr(&expr.into(), model)?;
            model.optimize(goal, variable)?;
        }
        flatzinc::Goal::OptimizeInt(optim, expr) => {
            let goal = goal_from_optim_type(&optim);
            let variable = basic_var_from_expr(&expr.into(), model)?;
            model.optimize(goal, variable)?;
        }
        _ => bail!("goal '{:?}' is not implemented", s_item.goal),
    };
    Ok(())
}
