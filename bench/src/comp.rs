use std::{fmt::Display, rc::Rc};

use crate::{Metric, SolveResult};
use owo_colors::OwoColorize;

enum Delta<T> {
    Better(T),
    Neutral(T),
    Worse(T),
    Unknown(T),
}
impl<T: Display> Display for Delta<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Delta::Better(value) => write!(f, "{}", value.to_string().green()),
            Delta::Neutral(value) => write!(f, "{}", value),
            Delta::Worse(value) => write!(f, "{}", value.to_string().red()),
            Delta::Unknown(value) => write!(f, "{}", value.to_string().blue()),
        }
    }
}

pub struct MeasureWithImprovement<T> {
    pub measure: Option<T>,
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
}

impl<T: Display> Display for MeasureWithImprovement<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Some(measure) = self.measure.as_ref() else {
            return write!(f, "-");
        };
        let Some(improv) = self.improvement.as_ref() else {
            return write!(f, "{}", Delta::Unknown(measure));
        };
        let res = if improv < &0.99 {
            Delta::Better(measure)
        } else if improv > &1.01 {
            Delta::Worse(measure)
        } else {
            Delta::Neutral(measure)
        };
        write!(f, "{res} (x{improv:.2})")
    }
}

fn increase<F>(lens: F, new: &SolveResult, reference: Option<&SolveResult>) -> String
where
    F: Fn(&SolveResult) -> f64,
{
    increase_fmt(lens(new), reference.map(lens))
}

fn increase_fmt(new: f64, base: Option<f64>) -> String {
    if let Some(reference) = base {
        let improv = new / reference;
        format!(" x{:.2}", improv)
    } else {
        "      ".to_string()
    }
}

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

    pub fn metric(&self, metric: Metric) -> MeasureWithImprovement<f64> {
        self.measure(|r| r.metrics.get(&metric).copied())
    }

    pub fn increase(&self, lens: impl Fn(&SolveResult) -> f64) -> String {
        increase_fmt(
            lens(self.run.as_ref()),
            self.reference.as_ref().map(|r| lens(r.as_ref())),
        )
    }
}

pub struct MetricValueWithRef {
    pub value: f64,
    pub reference: Option<f64>,
}

impl Display for MetricValueWithRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ", readable::Float::from(self.value))?;
        if let Some(reference) = self.reference {
            let improv = self.value / reference;
            write!(f, " x{:.2}", improv)
        } else {
            write!(f, "      ")
        }
    }
}
