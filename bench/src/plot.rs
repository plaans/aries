use itertools::Itertools;
use plotly::common::Mode;
use plotly::layout::{Axis, AxisType};
use plotly::{Layout, Plot, Scatter};
use std::collections::HashMap;

use crate::time_series::TimeSerie;

pub fn plot_cactus(series: &HashMap<impl AsRef<str>, TimeSerie>) {
    let mut plot = Plot::new();

    for (name, serie) in series.iter().sorted_by_key(|(name, _)| name.as_ref().to_string()) {
        let (xs, ys) = serie.line();
        let trace = Scatter::new(xs, ys).name(name).mode(Mode::Lines);
        plot.add_trace(trace);
    }
    let layout = Layout::new().x_axis(Axis::new().type_(AxisType::Log));
    plot.set_layout(layout);
    //plot.write_image("results/cactus.png", ImageFormat::PNG, 1500, 900, 1.0);
    plot.show();
}
