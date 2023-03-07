use std::convert::TryInto;

use anyhow::Context;
use malachite::Rational;
use unified_planning::{timepoint::TimepointKind, TimeInterval, Timing};

use crate::models::time::{TemporalInterval, Timepoint, TimepointKind as TimepointKindModel};

/* ========================================================================== */
/*                                 Conversion                                 */
/* ========================================================================== */

impl TryInto<Timepoint> for Timing {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<Timepoint, Self::Error> {
        let kind = match self.timepoint.context("Timing without timepoint")?.kind() {
            TimepointKind::GlobalStart => TimepointKindModel::GlobalStart,
            TimepointKind::GlobalEnd => TimepointKindModel::GlobalEnd,
            TimepointKind::Start => TimepointKindModel::Start,
            TimepointKind::End => TimepointKindModel::End,
        };
        let delay = self.delay.context("Timing without delay")?;
        let delay = Rational::from_signeds(delay.numerator, delay.denominator);
        Ok(Timepoint::new(kind, delay))
    }
}

impl TryInto<TemporalInterval> for TimeInterval {
    type Error = anyhow::Error;

    fn try_into(self) -> Result<TemporalInterval, Self::Error> {
        Ok(TemporalInterval::new(
            self.lower.context("Time interval without lower bound")?.try_into()?,
            self.upper.context("Time interval without upper bound")?.try_into()?,
            self.is_left_open,
            self.is_right_open,
        ))
    }
}

/* ========================================================================== */
/*                                    Tests                                   */
/* ========================================================================== */

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use unified_planning::Real;

    use crate::interfaces::unified_planning::factories::{time_interval::interval, timing::timing};

    use super::*;

    #[test]
    fn timing_to_timepoint() -> Result<()> {
        let delay = Real {
            numerator: 5,
            denominator: 2,
        };
        let d = Rational::from_signeds(5, 2);
        let gs = timing(TimepointKind::GlobalStart, delay.clone());
        let ge = timing(TimepointKind::GlobalEnd, delay.clone());
        let s = timing(TimepointKind::Start, delay.clone());
        let e = timing(TimepointKind::End, delay.clone());
        let mut nk = gs.clone();
        nk.timepoint = None;
        let mut nd = gs.clone();
        nd.delay = None;

        assert_eq!(
            Timepoint::new(TimepointKindModel::GlobalStart, d.clone()),
            gs.try_into()?
        );
        assert_eq!(Timepoint::new(TimepointKindModel::GlobalEnd, d.clone()), ge.try_into()?);
        assert_eq!(Timepoint::new(TimepointKindModel::Start, d.clone()), s.try_into()?);
        assert_eq!(Timepoint::new(TimepointKindModel::End, d.clone()), e.try_into()?);
        assert!(TryInto::<Timepoint>::try_into(nk).is_err());
        assert!(TryInto::<Timepoint>::try_into(nd).is_err());
        Ok(())
    }

    #[test]
    fn time_interval_to_temporal_interval() -> Result<()> {
        let s = timing(
            TimepointKind::GlobalStart,
            Real {
                numerator: 5,
                denominator: 2,
            },
        );
        let e = timing(
            TimepointKind::GlobalStart,
            Real {
                numerator: 7,
                denominator: 2,
            },
        );
        for u in [true, false] {
            for l in [true, false] {
                let ti = interval(s.clone(), e.clone(), l, u);
                let mut ns = ti.clone();
                ns.lower = None;
                let mut ne = ti.clone();
                ne.upper = None;
                assert_eq!(
                    TemporalInterval::new(s.clone().try_into()?, e.clone().try_into()?, l, u),
                    ti.try_into()?
                );
                assert!(TryInto::<TemporalInterval>::try_into(ns).is_err());
                assert!(TryInto::<TemporalInterval>::try_into(ne).is_err());
            }
        }
        Ok(())
    }
}
