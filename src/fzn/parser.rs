use std::rc::Rc;
use std::str::FromStr;

use anyhow::anyhow;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Context;
use flatzinc::Annotation;
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
use crate::fzn::model::Model;
use crate::fzn::solve::Goal;
use crate::fzn::types::Int;
use crate::fzn::var::BasicVar;
use crate::fzn::var::VarBool;
use crate::fzn::var::VarInt;

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
        Expr::VarParIdentifier(id) => model.get_var_bool(id),
        _ => bail!("not a varbool"),
    }
}

/// Return `true` iff the annotation asks for output.
///
/// Remark: it only check the annotation id.
pub fn is_output_anno(anno: &Annotation) -> bool {
    ["output_var", "output_array"].contains(&anno.id.as_str())
}

pub fn var_int_from_expr(
    expr: &Expr,
    model: &Model,
) -> anyhow::Result<Rc<VarInt>> {
    match expr {
        Expr::VarParIdentifier(id) => model.get_var_int(id),
        _ => bail!("not a varint"),
    }
}

pub fn basic_var_from_expr(
    expr: &Expr,
    model: &Model,
) -> anyhow::Result<BasicVar> {
    match expr {
        Expr::VarParIdentifier(id) => {
            model.get_variable(id)?.clone().try_into()
        }
        _ => bail!("not a basic var"),
    }
}

pub fn bool_from_expr(expr: &Expr, model: &Model) -> anyhow::Result<bool> {
    match expr {
        Expr::VarParIdentifier(id) => Ok(*model.get_par_bool(id)?.value()),
        Expr::Bool(b) => Ok(*b),
        _ => bail!("not a bool"),
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
            .cloned()
            .map(|e| var_bool_from_expr(&e.into(), model))
            .collect(),
        _ => todo!(),
    }
}

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
        _ => bail!("not an int vec"),
    }
}

pub fn vec_var_int_from_expr(
    expr: &Expr,
    model: &Model,
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

pub fn parse_model(content: impl Into<String>) -> anyhow::Result<Model> {
    let mut model = Model::new();

    let content = content.into();

    for (i, line) in content.lines().enumerate() {
        parse_line(line, &mut model)
            .context(format!("parsing failure at line {}", i + 1))?;
    }
    Ok(model)
}

pub fn parse_line(line: &str, model: &mut Model) -> anyhow::Result<()> {
    let statement = flatzinc::Stmt::from_str(line).map_err(|e| anyhow!(e))?;
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
        _ => todo!(),
    }
    Ok(())
}

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
        _ => todo!(),
    }
    Ok(())
}

pub fn parse_constraint_item(
    c: ConstraintItem,
    m: &mut Model,
) -> anyhow::Result<()> {
    let constraint = match c.id.as_str() {
        ArrayBoolAnd::NAME => ArrayBoolAnd::try_from_item(c, m)?.into(),
        ArrayIntMaximum::NAME => ArrayIntMaximum::try_from_item(c, m)?.into(),
        ArrayIntMinimum::NAME => ArrayIntMinimum::try_from_item(c, m)?.into(),
        BoolEq::NAME => BoolEq::try_from_item(c, m)?.into(),
        IntAbs::NAME => IntAbs::try_from_item(c, m)?.into(),
        IntEq::NAME => IntEq::try_from_item(c, m)?.into(),
        IntLe::NAME => IntLe::try_from_item(c, m)?.into(),
        IntLinEq::NAME => IntLinEq::try_from_item(c, m)?.into(),
        IntLinLe::NAME => IntLinLe::try_from_item(c, m)?.into(),
        IntLinNe::NAME => IntLinNe::try_from_item(c, m)?.into(),
        IntNe::NAME => IntNe::try_from_item(c, m)?.into(),
        _ => anyhow::bail!(format!("unknown constraint '{}'", c.id)),
    };
    m.add_constraint(constraint);
    Ok(())
}

// The optimize items do not rely on the type since flatzinc crate is not able to infer types for identifier
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

#[cfg(test)]
mod tests {
    use crate::fzn::domain::IntDomain;
    use crate::fzn::Name;

    use super::*;

    #[test]
    fn empty() -> anyhow::Result<()> {
        const CONTENT: &str = "% This is a comment\n\n";

        let model = parse_model(CONTENT)?;

        assert_eq!(model.nb_parameters(), 0);
        assert_eq!(model.nb_variables(), 0);
        assert_eq!(model.nb_constraints(), 0);

        Ok(())
    }

    #[test]
    fn parameters() -> anyhow::Result<()> {
        const CONTENT: &str = "int: x = 5;\nbool: y = true;";

        let model = parse_model(CONTENT)?;

        assert_eq!(model.nb_parameters(), 2);
        assert_eq!(model.nb_variables(), 0);
        assert_eq!(model.nb_constraints(), 0);

        let name_x = "x".to_string();
        let name_y = "y".to_string();

        let x = model.get_par_int(&name_x)?;
        let y = model.get_par_bool(&name_y)?;

        assert_eq!(*x.name(), name_x);
        assert_eq!(*x.value(), 5);

        assert_eq!(*y.name(), name_y);
        assert_eq!(*y.value(), true);

        Ok(())
    }

    #[test]
    fn int_variable() -> anyhow::Result<()> {
        const CONTENT: &str = "var -7..8: x;\n";

        let model = parse_model(CONTENT)?;

        assert_eq!(model.nb_parameters(), 0);
        assert_eq!(model.nb_variables(), 1);
        assert_eq!(model.nb_constraints(), 0);

        let name_x = "x".to_string();
        let domain_x: IntDomain = IntRange::new(-7, 8).unwrap().into();

        let x = model.get_var_int(&name_x)?;

        assert_eq!(x.name(), &name_x);
        assert_eq!(x.domain(), &domain_x);

        Ok(())
    }

    #[test]
    fn bool_eq() -> anyhow::Result<()> {
        const CONTENT: &str = "\
        var bool: x;\n\
        var bool: y;\n\
        constraint bool_eq(x,y);\n\
        ";

        let model = parse_model(CONTENT)?;

        assert_eq!(model.nb_parameters(), 0);
        assert_eq!(model.nb_variables(), 2);
        assert_eq!(model.nb_constraints(), 1);

        let name_x = Some("x".to_string());
        let name_y = Some("y".to_string());

        let x = model.get_var_bool(&name_x.unwrap())?;
        let y = model.get_var_bool(&name_y.unwrap())?;

        let c = model.constraints().next().unwrap();
        let bool_eq = BoolEq::try_from(c.clone())?;

        assert_eq!(bool_eq.a(), &x);
        assert_eq!(bool_eq.b(), &y);

        Ok(())
    }

    #[test]
    fn int_eq() -> anyhow::Result<()> {
        const CONTENT: &str = "\
        var 1..9: x;\n\
        var 0..2: y;\n\
        constraint int_eq(x,y);\n\
        ";

        let model = parse_model(CONTENT)?;

        assert_eq!(model.nb_parameters(), 0);
        assert_eq!(model.nb_variables(), 2);
        assert_eq!(model.nb_constraints(), 1);

        let domain_x: IntDomain = IntRange::new(1, 9)?.into();
        let domain_y: IntDomain = IntRange::new(0, 2)?.into();

        let name_x = "x".to_string();
        let name_y = "y".to_string();

        let x = model.get_var_int(&name_x)?;
        let y = model.get_var_int(&name_y)?;

        assert_eq!(x.domain(), &domain_x);
        assert_eq!(y.domain(), &domain_y);

        let c = model.constraints().next().unwrap();
        let int_eq = IntEq::try_from(c.clone())?;

        assert_eq!(int_eq.a(), &x);
        assert_eq!(int_eq.b(), &y);

        Ok(())
    }
}
