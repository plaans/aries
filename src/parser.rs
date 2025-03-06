use std::str::FromStr;

use anyhow::anyhow;
use anyhow::Context;
use flatzinc::ParDeclItem;
use flatzinc::Stmt;
use flatzinc::VarDeclItem;

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
        _ => todo!(),
    }
    Ok(())
}

pub fn parse_par_decl_item(par_decl_item: ParDeclItem, model: &mut Model) -> anyhow::Result<()>{
    match par_decl_item {
        ParDeclItem::Bool { id, bool } => {model.new_bool_parameter(id, bool)?;},
        ParDeclItem::Int { id, int } => {model.new_int_parameter(id, int.try_into()?)?;},
        _ => todo!(),
    }
    Ok(())
}

pub fn parse_var_decl_item(var_decl_item: VarDeclItem, model: &mut Model) -> anyhow::Result<()>{
    match var_decl_item {
        VarDeclItem::Bool { id, expr: _, annos: _ } => {model.new_bool_variable(id)?;},
        VarDeclItem::IntInRange { id, lb, ub, expr: _, annos: _ } => {
            let domain = IntRange::new(lb.try_into().unwrap(), ub.try_into().unwrap()).unwrap();
            model.new_int_variable(id, domain.into())?;
        },
        _ => todo!(),
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
        
        let x = model.get_int_parameter(&id_x)?;
        let y = model.get_bool_parameter(&id_y)?;

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
        
        let x = model.get_int_variable(&id_x)?;

        assert_eq!(*x.id(), id_x);
        assert_eq!(*x.domain(), domain_x);

        Ok(())
    }
}