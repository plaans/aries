use anyhow::Result;

use super::action::ValAction;

/// The minimal behaviour of a plan to validate it.
pub trait ValPlan {
    /// Returns an iterator of the plan actions.
    fn iter(&self) -> Result<ValPlanIter> {
        Ok(ValPlanIter {
            actions: self.actions()?,
            next: 0,
        })
    }

    /// Returns the actions of the plan.
    fn actions(&self) -> Result<Vec<Box<dyn ValAction>>>;
}

/// Iterator version of the ValPlan trait.
pub struct ValPlanIter {
    /// Actions of the plan.
    actions: Vec<Box<dyn ValAction>>,
    /// Current action index.
    next: usize,
}

impl Iterator for ValPlanIter {
    type Item = Box<dyn ValAction>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.actions.len() {
            None
        } else {
            self.next += 1;
            Some(dyn_clone::clone_box(&*self.actions[self.next - 1]))
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::*;

    #[derive(Clone)]
    struct Action(String);

    impl ValAction for Action {
        fn name(&self) -> anyhow::Result<String> {
            Ok(self.0.clone())
        }

        fn parameters(&self) -> Result<Vec<Box<dyn crate::models::expression::ValExpression>>> {
            todo!()
        }

        fn conditions(&self) -> Result<Vec<Box<dyn crate::models::condition::ValCondition>>> {
            todo!()
        }

        fn effects(&self) -> Result<Vec<Box<dyn crate::models::effect::ValEffect>>> {
            todo!()
        }
    }

    struct Plan(Vec<Action>);

    impl ValPlan for Plan {
        fn actions(&self) -> Result<Vec<Box<dyn ValAction>>> {
            Ok(self
                .0
                .iter()
                .map(|a| Box::new(a.clone()) as Box<dyn ValAction>)
                .collect::<Vec<_>>())
        }
    }

    #[test]
    fn iter() -> Result<()> {
        let plan_iter = Plan(vec![Action("a1".into()), Action("a2".into())]).iter()?;
        let iter = ValPlanIter {
            actions: vec![Box::new(Action("a1".into())), Box::new(Action("a2".into()))],
            next: 0,
        };
        assert_eq!(plan_iter.next, iter.next);
        assert_eq!(plan_iter.actions.len(), iter.actions.len());
        for i in 0..plan_iter.actions.len() {
            assert_eq!(
                plan_iter.actions.get(i).unwrap().name()?,
                iter.actions.get(i).unwrap().name()?
            );
        }
        Ok(())
    }

    #[test]
    fn next() -> Result<()> {
        let mut i = 0;
        for action in Plan(vec![Action("a1".into()), Action("a2".into())]).iter()? {
            i += 1;
            assert_eq!(action.name()?, format!("a{:?}", i));
        }
        Ok(())
    }
}
