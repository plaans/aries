use anyhow::Result;

pub struct IntVariable {
    id: String,
    lb: i32,
    ub: i32,
}

impl IntVariable {

    /// Build a new `IntVariable` with the given identifier, lower bound and upper bound.
    /// 
    /// Return an `Err` in the following cases:
    ///  - the identifier is empty
    ///  - the lower bound is greater than the upper bound
    pub fn new(id: String, lb: i32, ub: i32) -> Result<Self> {
        anyhow::ensure!(!id.is_empty(), "id is empty");
        anyhow::ensure!(lb <= ub, "lb is greater than ub ({lb} > {ub})");
        Ok(IntVariable{id, lb, ub})
    }

    /// Return the variable identifier.
    pub fn id(self: &Self) -> &str {
        &self.id
    }

    /// Return the variable lower bound.
    pub fn lb(self: &Self) -> &i32 {
        &self.lb
    }
    
    /// Return the variable upper bound.
    pub fn ub(self: &Self) -> &i32 {
        &self.ub
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_and_getters() {
        let attrs_ok = [
            ("x", -5, 2,  true),
            ( "", -5, 2, false),
            ( "",  5, 2, false),
            ("x",  5, 2, false),
        ];
        for (id, lb, ub, ok) in attrs_ok {
            let var = IntVariable::new(id.to_string(), lb, ub);
            if ok {
                let var = var.expect("result should be Ok");
                assert_eq!(var.id(), id);
                assert_eq!(*var.lb(), lb);
                assert_eq!(*var.ub(), ub);
            } else {
                assert!(var.is_err());
            }
        }
    }
}