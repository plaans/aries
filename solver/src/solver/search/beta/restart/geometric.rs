use crate::backtrack::DecLvl;
use crate::core::state::Conflict;
use crate::core::state::Explainer;
use crate::model::Label;
use crate::model::Model;
use crate::solver::search::beta::restart::Restart;

#[derive(Clone, Debug)]
pub struct Geometric {
    scaling_factor: f32,
    period: u32,
    countdown: u32,
}

impl Geometric {
    pub fn new(scaling_factor: f32, period: u32) -> Self {
        debug_assert!(scaling_factor >= 1.0);
        debug_assert!(period >= 1);
        Geometric {
            scaling_factor,
            period,
            countdown: period,
        }
    }

    /// Decay the variable activity.
    fn scale(&mut self) {
        self.period = (self.scaling_factor * self.period as f32) as u32;
    }
}

impl<Lbl: Label> Restart<Lbl> for Geometric {
    fn conflict(
        &mut self,
        _clause: &Conflict,
        _model: &Model<Lbl>,
        _explainer: &mut dyn Explainer,
        _backtrack_level: DecLvl,
    ) {
        self.countdown = self.countdown.saturating_sub(1);
        debug_assert!(self.countdown <= self.period);
    }

    fn restart(&mut self) -> bool {
        let timesup = self.countdown == 0;
        if timesup {
            self.scale();
            self.countdown = self.period;
        }
        timesup
    }
}

impl Default for Geometric {
    fn default() -> Self {
        Self::new(1.1, 100)
    }
}
