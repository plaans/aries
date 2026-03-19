use std::{fmt::Display, rc::Rc};

use crate::{SolveResult, SolverMetric};

/// An optional measure, together with an optional improvement ratio if we had
/// the corresponding measure in the reference set.
#[derive(Debug)]
pub struct MeasureWithImprovement<T> {
    /// Main measure for the current run.
    /// May be absent.
    pub measure: Option<T>,
    /// Improvement: ratio of the new measure and the old one.
    /// Thus a number > 1 indicates and increase.
    pub improvement: Option<f64>,
}
impl<T> MeasureWithImprovement<T> {
    pub fn absent() -> Self {
        MeasureWithImprovement {
            measure: None,
            improvement: None,
        }
    }

    pub fn map<To>(self, f: impl FnOnce(T) -> To) -> MeasureWithImprovement<To> {
        MeasureWithImprovement {
            measure: self.measure.map(f),
            improvement: self.improvement,
        }
    }
    pub fn cell(self) -> comfy_table::Cell
    where
        T: Display,
    {
        let color = match self.improvement {
            None => comfy_table::Color::White,
            Some(n) if n > 1.0001 => comfy_table::Color::Red,
            Some(n) if n < 0.9999 => comfy_table::Color::Green,
            _ => comfy_table::Color::Grey,
        };
        comfy_table::Cell::new(self)
            .fg(color)
            .set_alignment(comfy_table::CellAlignment::Right)
    }
}

impl<T: Display> Display for MeasureWithImprovement<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Some(measure) = self.measure.as_ref() else {
            return write!(f, "-        ");
        };
        let Some(improv) = self.improvement.as_ref() else {
            return write!(f, "{measure}        ");
        };
        write!(f, "{measure} (x{improv:.2})")
    }
}

/// A solver result on a given problem, along with corresponding run of the reference (if any)
#[derive(Debug)]
pub struct RunWithRef {
    pub run: Rc<SolveResult>,
    pub reference: Option<Rc<SolveResult>>,
}

impl RunWithRef {
    pub fn objective(&self) -> MeasureWithImprovement<i32> {
        self.measure(|r| r.objective_value.map(|o| o as i32))
    }
    pub fn measure<T: Copy + Into<f64>>(&self, lens: impl Fn(&SolveResult) -> Option<T>) -> MeasureWithImprovement<T> {
        let Some(new) = lens(self.run.as_ref()) else {
            return MeasureWithImprovement::absent();
        };
        let previous = self.reference.as_ref().and_then(|r| lens(r.as_ref()));
        MeasureWithImprovement {
            measure: Some(new),
            improvement: previous.map(|prev| new.into() / prev.into()),
        }
    }

    pub fn metric(&self, metric: SolverMetric) -> MeasureWithImprovement<f64> {
        self.measure(|r| r.metrics.get(&metric).copied())
    }
}
