use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Context;
use flatzinc::ConstraintItem;
use flatzinc::ParDeclItem;
use flatzinc::Stmt;
use flatzinc::VarDeclItem;

use crate::adapter::bool_and_from_item;
use crate::adapter::int_eq_from_item;
use crate::constraint::builtins::BoolAnd;
use crate::constraint::builtins::IntEq;
use crate::domain::IntRange;
use crate::model::Model;

pub fn parse_model(content: impl Into<String>) -> anyhow::Result<Model> {
    let mut model = Model::new();

    let content = content.into();

    for (i, line) in content.lines().enumerate() {
        println!(">> {line}");
        parse_line(&line, &mut model).context(format!("parsing failure at line {i}"))?;
        println!(" OK\n");
    }
    Ok(model)
}

pub fn parse_line(line: &str, model: &mut Model) -> anyhow::Result<()> {
    let statement = flatzinc::Stmt::from_str(&line).map_err(|e| anyhow!(e))?;
    match statement {
        Stmt::Comment(_) => {},
        Stmt::Parameter(par_decl_item) => parse_par_decl_item(par_decl_item, model)?,
        Stmt::Variable(var_decl_item) => parse_var_decl_item(var_decl_item, model)?,
        Stmt::Constraint(constraint_item) => parse_constraint_item(constraint_item, model)?,
        _ => todo!(),
    }
    Ok(())
}

pub fn parse_par_decl_item(par_decl_item: ParDeclItem, model: &mut Model) -> anyhow::Result<()> {
    match par_decl_item {
        ParDeclItem::Bool { id, bool } => {model.new_par_bool(id, bool)?;},
        ParDeclItem::Int { id, int } => {model.new_par_int(id, int.try_into()?)?;},
        _ => todo!(),
    }
    Ok(())
}

pub fn parse_var_decl_item(var_decl_item: VarDeclItem, model: &mut Model) -> anyhow::Result<()> {
    match var_decl_item {
        VarDeclItem::Bool { id, expr: _, annos: _ } => {model.new_var_bool(id)?;},
        VarDeclItem::IntInRange { id, lb, ub, expr: _, annos: _ } => {
            let domain = IntRange::new(lb.try_into().unwrap(), ub.try_into().unwrap()).unwrap();
            model.new_var_int(id, domain.into())?;
        },
        _ => todo!(),
    }
    Ok(())
}

pub fn parse_constraint_item(c_item: ConstraintItem, model: &mut Model) -> anyhow::Result<()> {
    match c_item.id.as_str() {
        BoolAnd::NAME => {model.add_constraint(bool_and_from_item(c_item, model)?.into())?;},
        IntEq::NAME => {model.add_constraint(int_eq_from_item(c_item, model)?.into())?;},
        _ => anyhow::bail!(format!("unkown constraint '{}'", c_item.id)),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::domain::IntDomain;
    use crate::traits::Identifiable;

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

        let id_x = "x".to_string();
        let id_y = "y".to_string();
        
        let x = model.get_par_int(&id_x)?;
        let y = model.get_par_bool(&id_y)?;

        assert_eq!(*x.id(), id_x);
        assert_eq!(*x.value(), 5);

        assert_eq!(*y.id(), id_y);
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

        let id_x = "x".to_string();
        let domain_x: IntDomain = IntRange::new(-7, 8).unwrap().into();
        
        let x = model.get_var_int(&id_x)?;

        assert_eq!(*x.id(), id_x);
        assert_eq!(*x.domain(), domain_x);

        Ok(())
    }

    #[test]
    fn bool_and() -> anyhow::Result<()> {
        const CONTENT: &str = "\
        var bool: x;\n\
        var bool: y;\n\
        constraint bool_and(x,y);\n\
        ";

        let model = parse_model(CONTENT)?;

        assert_eq!(model.nb_parameters(), 0);
        assert_eq!(model.nb_variables(), 2);
        assert_eq!(model.nb_constraints(), 1);

        let id_x = "x".to_string();
        let id_y = "y".to_string();

        let x = model.get_var_bool(&id_x)?;
        let y = model.get_var_bool(&id_y)?;

        let c = model.constraints().next().unwrap();
        let bool_and = BoolAnd::try_from(c.clone())?;

        assert_eq!(bool_and.a(), &x);
        assert_eq!(bool_and.b(), &y);

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

        let id_x = "x".to_string();
        let id_y = "y".to_string();

        let x = model.get_var_int(&id_x)?;
        let y = model.get_var_int(&id_y)?;

        assert_eq!(x.domain(), &domain_x);
        assert_eq!(y.domain(), &domain_y);

        let c = model.constraints().next().unwrap();
        let int_eq = IntEq::try_from(c.clone())?;

        assert_eq!(int_eq.a(), &x);
        assert_eq!(int_eq.b(), &y);

        Ok(())
    }
}