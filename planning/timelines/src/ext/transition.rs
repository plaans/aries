use aries::prelude::Lit;

use crate::constraints::HasValueAt;
use crate::encoder::CondId;
use crate::ext::encoder::Source;
use crate::{Effect, EffectId, IntTerm, StateVar};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransitionId {
    Cond(CondId),
    Eff(EffectId),
    /// A condition and effect sharing the same source, presence literal, and state variable.
    CondEff(CondId, EffectId),
}

#[derive(PartialEq, Eq)]
pub enum TransitionRef<'a> {
    Cond(&'a HasValueAt),
    Eff(&'a Effect),
    CondEff(&'a HasValueAt, &'a Effect),
}
impl<'a> TransitionRef<'a> {
    pub fn get_condition(&self) -> Option<&'a HasValueAt> {
        match self {
            TransitionRef::Eff(_) => None,
            TransitionRef::Cond(c) | TransitionRef::CondEff(c, _) => Some(c),
        }
    }
    pub fn get_effect(&self) -> Option<&'a Effect> {
        match self {
            TransitionRef::Cond(_) => None,
            TransitionRef::Eff(e) | TransitionRef::CondEff(_, e) => Some(e),
        }
    }
    pub fn get_source(&self) -> Source {
        match self {
            TransitionRef::Cond(c) => c.source,
            TransitionRef::Eff(e) => e.source,
            TransitionRef::CondEff(c, e) => {
                debug_assert!(c.source == e.source);
                c.source
            }
        }
    }
    pub fn get_prez(&self) -> Lit {
        match self {
            TransitionRef::Cond(c) => c.prez,
            TransitionRef::Eff(e) => e.prez,
            TransitionRef::CondEff(c, e) => {
                debug_assert!(c.prez == e.prez);
                c.prez
            }
        }
    }
    pub fn get_state_var(&self) -> &'a StateVar {
        match self {
            TransitionRef::Cond(c) => &c.state_var,
            TransitionRef::Eff(e) => &e.state_var,
            TransitionRef::CondEff(c, e) => {
                debug_assert!(c.state_var == e.state_var);
                &c.state_var
            }
        }
    }
    pub fn get_args(&self) -> &'a [IntTerm] {
        &self.get_state_var().args
    }
    pub fn get_valfrom(&self) -> Option<&IntTerm> {
        self.get_condition().map(|c| &c.value)
    }
    pub fn get_valto(&self) -> Option<&IntTerm> {
        self.get_effect().map(|e| match &e.operation {
            crate::EffectOp::Assign(term) => term,
            crate::EffectOp::Step(_) => todo!(),
        })
    }
    pub fn get_terms(&self) -> (&'a [IntTerm], Option<&IntTerm>, Option<&IntTerm>) {
        debug_assert!(self.get_valfrom().is_some() || self.get_valto().is_some());
        (self.get_args(), self.get_valfrom(), self.get_valto())
    }
    pub fn terms_len(&self) -> usize {
        self.get_args().len() + self.get_valfrom().is_some() as usize + self.get_valto().is_some() as usize
    }
    pub fn get_term(&self, i: usize) -> &IntTerm {
        if i == self.get_args().len() + 1 {
            self.get_valto().unwrap()
        } else if i == self.get_args().len() {
            self.get_valfrom().unwrap()
        } else {
            &self.get_args()[i]
        }
    }
    pub fn get_valfrom_idx(&self) -> Option<usize> {
        match self {
            TransitionRef::Cond(_) => Some(self.get_args().len()),
            TransitionRef::Eff(_) => None,
            TransitionRef::CondEff(_, _) => Some(self.get_args().len()),
        }
    }
    pub fn get_valto_idx(&self) -> Option<usize> {
        match self {
            TransitionRef::Cond(_) => None,
            TransitionRef::Eff(_) => Some(self.get_args().len()),
            TransitionRef::CondEff(_, _) => Some(self.get_args().len() + 1),
        }
    }
    /*pub fn terms_iter(&self) -> impl Iterator<Item = &IntTerm> {
        self.args().iter().chain(self.valfrom()).chain(self.valto())
    }*/
    pub fn iter_terms(&self) -> impl Iterator<Item = IntTerm> + use<'_> {
        self.get_args()
            .iter()
            .copied()
            .chain(self.get_valfrom().copied())
            .chain(self.get_valto().copied())
    }
}

impl<'a> std::fmt::Debug for TransitionRef<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "[{:?}]({:?}: {:?}->{:?})",
            self.get_source(),
            self.get_state_var(),
            self.get_valfrom(),
            self.get_valto(),
        )
    }
}
