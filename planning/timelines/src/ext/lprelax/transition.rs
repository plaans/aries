use aries::prelude::Lit;

use crate::constraints::HasValueAt;
use crate::encoder::CondId;
use crate::ext::lprelax::Source;
use crate::{Effect, EffectId, IntTerm, StateVar};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum Transition {
    Cond(CondId),
    Eff(EffectId),
    /// A condition and effect sharing the same source, presence literal, and state variable.
    CondEff(CondId, EffectId),
}
pub type TransitionId = usize;

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
    pub fn get_valfrom(&self) -> Option<&'a IntTerm> {
        self.get_condition().map(|c| &c.value)
    }
    pub fn get_valto(&self) -> Option<&'a IntTerm> {
        self.get_effect().map(|e| match &e.operation {
            crate::EffectOp::Assign(term) => term,
            crate::EffectOp::Step(_) => todo!(),
        })
    }
    #[allow(dead_code)]
    pub fn get_terms(&self) -> (&'a [IntTerm], Option<&IntTerm>, Option<&IntTerm>) {
        debug_assert!(self.get_valfrom().is_some() || self.get_valto().is_some());
        (self.get_args(), self.get_valfrom(), self.get_valto())
    }
    pub fn terms_len(&self) -> usize {
        self.get_args().len() + self.get_valfrom().is_some() as usize + self.get_valto().is_some() as usize
    }
    pub fn get_term(&self, i: usize) -> &'a IntTerm {
        if i < self.get_args().len() {
            &self.get_args()[i]
        } else if i == self.get_args().len() {
            match self {
                TransitionRef::Cond(_) | TransitionRef::CondEff(_, _) => self.get_valfrom().unwrap(),
                TransitionRef::Eff(_) => self.get_valto().unwrap(),
            }
        } else if i == self.get_args().len() + 1 {
            match self {
                TransitionRef::CondEff(_, _) => self.get_valto().unwrap(),
                _ => panic!(),
            }
        } else {
            panic!()
        }
    }
    pub fn get_valfrom_term_idx(&self) -> Option<usize> {
        match self {
            TransitionRef::Cond(_) => Some(self.get_args().len()),
            TransitionRef::Eff(_) => None,
            TransitionRef::CondEff(_, _) => Some(self.get_args().len()),
        }
    }
    pub fn get_valto_term_idx(&self) -> Option<usize> {
        match self {
            TransitionRef::Cond(_) => None,
            TransitionRef::Eff(_) => Some(self.get_args().len()),
            TransitionRef::CondEff(_, _) => Some(self.get_args().len() + 1),
        }
    }
    pub fn iter_terms(&self) -> impl Iterator<Item = &'a IntTerm> {
        self.get_args().iter().chain(self.get_valfrom()).chain(self.get_valto())
    }
    pub fn _iter_terms_move(self) -> impl Iterator<Item = &'a IntTerm> {
        TransitionTermsIter { tr: self, next: 0 }
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

struct TransitionTermsIter<'a> {
    tr: TransitionRef<'a>,
    next: usize,
}

impl<'a> Iterator for TransitionTermsIter<'a> {
    type Item = &'a IntTerm;

    fn next(&mut self) -> Option<Self::Item> {
        if self.next >= self.tr.terms_len() {
            None
        } else {
            let i = self.next;
            self.next += 1;
            Some(self.tr.get_term(i))
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let rem = self.tr.terms_len() - self.next;
        (rem, Some(rem))
    }
}

impl ExactSizeIterator for TransitionTermsIter<'_> {}
