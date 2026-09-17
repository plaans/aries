use std::collections::HashMap;

use aries_solver::core::IntCst;
use itertools::Itertools;

use crate::analysis::Source;
use crate::constraints::lprelax::LpRelaxEncoder;
use crate::constraints::lprelax::encoder::groundings::SourceGroundingId;
use crate::constraints::lprelax::transitions::TransitionId;
use crate::constraints::lprelax::transitions::ground::TransitionGroundingId;
use crate::{Domains, IntTerm, encoder::SchedEncoder};

/// Represents a variable / column
#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord, Hash)]
pub enum ColTag {
    PresenceSource(Source, Option<SourceGroundingId>),
    PresenceTransition(TransitionId, Option<TransitionGroundingId>),
    Support(
        TransitionId,
        TransitionId,
        Option<(TransitionGroundingId, TransitionGroundingId)>,
    ),
    TermGround(IntTerm, IntCst),
}

/// Represents an expression of the form `lhs cmp rhs + cst`.
/// where `cmp` can be `=`, `=`, or `>=`, and coefficient of terms in `lhs` and `rhs` is 1.
#[derive(Debug, Clone)]
pub struct RowExpr {
    pub tpe: RowExprType,
    /// Index separating the lhs and rhs terms. As such, `terms[separator]` must be the first term of the rhs.
    separator: usize,
    terms: Vec<ColTag>,
    /// Constant part of the expression, in the rhs
    cst: IntCst,
}
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum RowExprType {
    Eq,
    Leq,
    Geq,
}
impl RowExpr {
    pub fn lhs(&self) -> &[ColTag] {
        self.terms.get(..self.separator).unwrap_or(&[])
    }
    pub fn rhs(&self) -> &[ColTag] {
        self.terms.get(self.separator..).unwrap_or(&[])
    }
    pub fn cst(&self) -> IntCst {
        self.cst
    }
    #[allow(dead_code)]
    pub fn new(tpe: RowExprType, terms: Vec<ColTag>, separator: usize, cst: IntCst) -> Self {
        Self {
            tpe,
            separator,
            terms,
            cst,
        }
    }
    pub fn new_eq_single_lhs(terms: Vec<ColTag>) -> Self {
        Self {
            tpe: RowExprType::Eq,
            separator: 1,
            terms,
            cst: 0,
        }
    }
    pub fn new_leq_single_lhs(terms: Vec<ColTag>) -> Self {
        Self {
            tpe: RowExprType::Leq,
            separator: 1,
            terms,
            cst: 0,
        }
    }
    pub fn new_geq_single_lhs(terms: Vec<ColTag>) -> Self {
        Self {
            tpe: RowExprType::Geq,
            separator: 1,
            terms,
            cst: 0,
        }
    }
    pub fn new_leq_1(terms: Vec<ColTag>) -> Self {
        Self {
            tpe: RowExprType::Leq,
            separator: terms.len(),
            terms,
            cst: 1,
        }
    }
}

/// Represents a problem
#[derive(Clone, Default)]
pub struct LpRelaxProblem {
    cols: Option<Vec<ColTag>>,
    rows: Vec<RowExpr>,
}
impl LpRelaxProblem {
    pub fn push_row(&mut self, row: RowExpr) {
        self.cols = None;
        self.rows.push(row);
    }
    pub fn rows(&self) -> &[RowExpr] {
        &self.rows
    }
    pub fn cols(&self) -> Option<&[ColTag]> {
        self.cols.as_deref()
    }
    pub fn sealed(&self) -> bool {
        self.cols.is_some()
    }
    fn seal(&mut self) {
        self.cols = Some(
            self.rows
                .iter()
                .flat_map(|row| row.terms.iter())
                .unique()
                .copied()
                .collect(),
        );
    }
    /// Propagate (some) columns (based on domains) by replacing their occurrences (in rows) with values.
    /// Remove consistent empty rows.
    /// Remove (some) consistent single-term equality rows.
    ///
    /// If an inconsistent empty or single-term equality row is detected, make the problem empty with a single column and a single trivially infeasible row.
    pub fn simplify(&mut self, encoder: &LpRelaxEncoder, ctx: &SchedEncoder, doms: &Domains) {
        let make_inconsistent = |cols_vec: &mut Option<Vec<_>>, rows: &mut Vec<_>| {
            *cols_vec = Some(vec![ColTag::PresenceSource(None, None)]);
            *rows = vec![RowExpr::new(
                RowExprType::Geq,
                vec![ColTag::PresenceSource(None, None)],
                1,
                2,
            )];
        };

        // Start by collecting propagated values (if any) for
        // lifted presence columns, as well as (all potential) lifted (original causal link) supports columns.

        let mut propagated_cols_map = {
            let starting_candidate_cols = encoder
                .iter_sources()
                .flat_map(|(source, transitions_ids)| {
                    let col_tag = ColTag::PresenceSource(source, None);
                    std::iter::chain(
                        [(col_tag, encoder.get_source_prez(source, ctx))],
                        transitions_ids.iter().map(|&transition_id| {
                            let col_tag = ColTag::PresenceTransition(transition_id, None);
                            (col_tag, encoder.transitions.get_prez(transition_id, ctx))
                        }),
                    )
                })
                .chain(encoder.supports.sorted_out().iter().filter_map(
                    |&((out_transition_id, in_transition_id), active)| {
                        active.map(|active| {
                            let col_tag = ColTag::Support(out_transition_id, in_transition_id, None);
                            (col_tag, active)
                        })
                    },
                ));
            let mut res = HashMap::new();

            for (col_tag, lit) in starting_candidate_cols {
                if doms.entails(doms.presence(lit)) {
                    if let Some(v) = doms.value(lit).map(|vv| if vv { 1 } else { 0 }) {
                        debug_assert!(res.get(&col_tag).is_none_or(|vv| *vv == v));
                        res.insert(col_tag, v);
                    }
                } else if doms.entails(!doms.presence(lit)) {
                    debug_assert!(res.get(&col_tag).is_none_or(|v| *v == 0));
                    res.insert(col_tag, 0);
                }
            }
            res
        };

        let attempt_propagation_to_col_if_term_grounding =
            |col_tag: &ColTag, propagated_cols_map: &mut HashMap<_, _>| -> Result<(), ()> {
                let &ColTag::TermGround(term, value) = col_tag else {
                    return Ok(());
                };

                if doms.entails(doms.presence(term)) {
                    if value < doms.lb(term) || doms.ub(term) < value {
                        if let Some(v) = propagated_cols_map.get_mut(col_tag) {
                            if *v != 0 {
                                return Err(());
                            }
                        } else {
                            propagated_cols_map.insert(*col_tag, 0);
                        }
                    } else if value == doms.lb(term) && doms.lb(term) == doms.ub(term) {
                        if let Some(v) = propagated_cols_map.get_mut(col_tag) {
                            if *v != 1 {
                                return Err(());
                            }
                        } else {
                            propagated_cols_map.insert(*col_tag, 1);
                        }
                    }
                } else if doms.entails(!doms.presence(term)) {
                    if let Some(v) = propagated_cols_map.get_mut(col_tag) {
                        if *v != 0 {
                            return Err(());
                        }
                    } else {
                        propagated_cols_map.insert(*col_tag, 0);
                    }
                }

                Ok(())
            };

        // Iterate over row expressions and:
        // - on encountering a term grounding column, check if it's propagated and if so, intern it as such
        // - on encountering a propagated term, replace it by its value in the row

        loop {
            let old_propagated_cols_map_len = propagated_cols_map.len();
            let old_rows_len = self.rows.len();

            for row in &mut self.rows {
                let separator = row.separator;
                let mut new_separator = separator;
                let mut kept_terms = Vec::with_capacity(row.terms.len());

                for (i, col_tag) in std::mem::take(&mut row.terms).into_iter().enumerate() {
                    if attempt_propagation_to_col_if_term_grounding(&col_tag, &mut propagated_cols_map).is_err() {
                        make_inconsistent(&mut self.cols, &mut self.rows);
                        return;
                    }

                    match propagated_cols_map.get(&col_tag) {
                        // `lhs cmp rhs + cst`, so a known lhs term moves right (-) and a known rhs term remain on the right (+).
                        Some(v) => {
                            if i < separator {
                                row.cst -= v;
                                new_separator -= 1;
                            } else {
                                row.cst += v;
                            }
                        }
                        None => kept_terms.push(col_tag),
                    }
                }
                row.terms = kept_terms;
                row.separator = new_separator;

                if row.terms.is_empty()
                    && match row.tpe {
                        RowExprType::Eq => 0 != row.cst,
                        RowExprType::Leq => 0 > row.cst,
                        RowExprType::Geq => 0 < row.cst,
                    }
                {
                    make_inconsistent(&mut self.cols, &mut self.rows);
                    return;
                }

                if row.terms.len() == 1 {
                    let col_tag = *row.terms.first().unwrap();
                    let on_lhs = row.separator >= 1;

                    // On the lhs the row reads `x cmp cst`; on the rhs it reads `0 cmp x + cst`,
                    // i.e. `x cmp -cst` with the comparison reversed.
                    let inconsistent_with = |value: IntCst| {
                        if on_lhs {
                            match row.tpe {
                                RowExprType::Eq => value != row.cst,
                                RowExprType::Leq => value > row.cst,
                                RowExprType::Geq => value < row.cst,
                            }
                        } else {
                            match row.tpe {
                                RowExprType::Eq => value != -row.cst,
                                RowExprType::Leq => value < -row.cst,
                                RowExprType::Geq => value > -row.cst,
                            }
                        }
                    };

                    match propagated_cols_map.get(&col_tag).copied() {
                        Some(v) => {
                            if inconsistent_with(v) {
                                make_inconsistent(&mut self.cols, &mut self.rows);
                                return;
                            }
                            if row.tpe == RowExprType::Eq {
                                if on_lhs {
                                    row.cst -= v
                                } else {
                                    row.cst += v
                                }
                                row.terms.clear();
                                row.separator = 0;
                            }
                        }
                        None => {
                            if row.tpe == RowExprType::Eq {
                                let value = if on_lhs { row.cst } else { -row.cst };
                                if !(0..=1).contains(&value) {
                                    // the column lives in [0, 1], so no assignment satisfies this row
                                    make_inconsistent(&mut self.cols, &mut self.rows);
                                    return;
                                }
                                propagated_cols_map.insert(col_tag, value);
                            }
                        }
                    }
                }
            }

            // Remove:
            // - empty *consistent* rows
            // - as many empty *inconsistent* rows, except at least one ! (we need to keep it to still be able to derive the (trivial) infeasibility)
            // - single-termed equalites *consistent* rows (again, )

            let mut kept_rows = Vec::with_capacity(self.rows.len());

            for row in std::mem::take(&mut self.rows) {
                if row.terms.is_empty() {
                    continue;
                }
                kept_rows.push(row);
            }
            self.rows = kept_rows;

            if old_rows_len == self.rows.len() && old_propagated_cols_map_len == propagated_cols_map.len() {
                break;
            }
        }

        self.seal();
    }
}
