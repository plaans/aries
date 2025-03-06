use std::str::FromStr;

use anyhow::anyhow;
use flatzinc::ParDeclItem;
use flatzinc::Stmt;

use crate::model::Model;

pub fn parse_model(content: impl Into<String>) -> anyhow::Result<Model> {
    let mut model = Model::new();

    let content = content.into();

    for line in content.lines() {
        parse_line(&line, &mut model)?;
    }
    Ok(model)
}

pub fn parse_line(line: &str, model: &mut Model) -> anyhow::Result<()> {
    let statement = flatzinc::Stmt::from_str(&line).map_err(|e| anyhow!(e))?;
    match statement {
        Stmt::Comment(_) => {},
        Stmt::Parameter(par_decl_item) => parse_par_decl_item(par_decl_item, model)?,
        _ => todo!(),
    }
    Ok(())
}

pub fn parse_par_decl_item(par_decl_item: ParDeclItem, model: &mut Model) -> anyhow::Result<()>{
    match par_decl_item {
        ParDeclItem::Bool { id, bool } => model.new_bool_parameter(id, bool).map(|_| ()),
        ParDeclItem::Int { id, int } => model.new_int_parameter(id, int.try_into()?).map(|_| ()),
        _ => todo!(),
    }
}

#[cfg(test)]
mod tests {
    use crate::traits::Identifiable;

    use super::*;

    #[test]
    fn empty() {
        const CONTENT: &str = "% This is a comment\n\n";
        let model = parse_model(CONTENT).unwrap();
        assert_eq!(model.nb_parameters(), 0);
        assert_eq!(model.nb_variables(), 0);
        assert_eq!(model.nb_constraints(), 0);
    }

    #[test]
    fn parameters() {
        const CONTENT: &str = "int: x = 5;\nbool: y = true;";
        let model = parse_model(CONTENT).unwrap();

        assert_eq!(model.nb_parameters(), 2);
        assert_eq!(model.nb_variables(), 0);
        assert_eq!(model.nb_constraints(), 0);

        let id_x = "x".to_string();
        let id_y = "y".to_string();
        
        let x = model.get_int_parameter(&id_x).unwrap();
        let y = model.get_bool_parameter(&id_y).unwrap();

        assert_eq!(*x.id(), id_x);
        assert_eq!(*x.value(), 5);

        assert_eq!(*y.id(), id_y);
        assert_eq!(*y.value(), true);
    }
}