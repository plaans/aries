use derive_builder::Builder;
use itertools::Itertools;
use std::{collections::BTreeMap, path::Path};

use crate::time_series::TimeSerie;

#[derive(Clone, Builder)]
#[builder(pattern = "owned")]
#[builder(setter(strip_option))]
#[builder(derive(Clone))]
pub struct PlotOptions {
    #[builder(setter(strip_option))]
    pub title: Option<String>,
    #[builder(setter(strip_option))]
    pub x_label: Option<String>,
    #[builder(setter(strip_option))]
    pub y_label: Option<String>,
    #[builder(setter(strip_option))]
    pub min_x: Option<f64>,
    #[builder(default = "false")]
    pub log_x: bool,
    #[builder(default = "\"/tmp/plots\".to_string()")]
    pub out_dir: String,
    #[builder(setter(strip_option))]
    pub file: Option<String>,
    #[builder(default = "(5.0, 5.0)")]
    pub dimensions: (f32, f32),
    #[builder(default = "LegendLoc::TopLeft")]
    pub legenc_loc: LegendLoc,
}
impl Default for PlotOptions {
    fn default() -> Self {
        Self {
            title: None,
            min_x: None,
            log_x: false,
            x_label: None,
            y_label: None,
            out_dir: String::new(),
            file: None,
            dimensions: (4.0, 4.0),
            legenc_loc: LegendLoc::TopLeft,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LegendLoc {
    None,
    TopLeft,
    BottomRight,
}

fn when<T, R>(opt: &Option<T>, f: impl FnOnce(&T) -> R) {
    if let Some(elem) = opt.as_ref() {
        f(elem);
    }
}

pub fn plot_cactus(series: &BTreeMap<impl AsRef<str>, TimeSerie>, options: &PlotOptions) {
    // let colors = ["red", "blue", "cyan", "orange"];
    let colors = ["#d31f11", "#007191", "#f47a00", "#62c8d3"];
    let colors = colors.map(|c| Color(c.into()));
    let styles = [LineStyle(Solid), LineStyle(Dash), LineStyle(Dot)];
    use gnuplot::*;

    // combination of a group (color) and line style
    let confs = [(0, 0), (1, 1), (2, 2), (0, 1), (1, 2), (2, 0), (0, 2), (1, 0), (2, 1)];

    let mut fg = Figure::new();
    let ax = fg.axes2d();
    for (i, (name, serie)) in series
        .iter()
        .sorted_by_key(|(name, _)| name.as_ref().to_string())
        .enumerate()
    {
        let name = name.as_ref();
        let (group, place) = confs[i];
        let color = colors[group].clone();
        let style = styles[place].clone();
        let (xs, ys) = serie.line();
        ax
            // .lines(&xs, &ys, &[Caption(name.as_ref()), Color("black".into())]);
            .lines(&xs, &ys, &[Caption(name), PointSize(1.5), LineWidth(1.5), color, style]);
    }
    if options.log_x {
        ax.set_x_log(Some(10.0)).set_x_label("Time (s)", &[]);
    }
    when(&options.title, |t| ax.set_title(t, &[]));
    when(&options.x_label, |lbl| ax.set_x_label(lbl, &[]));
    when(&options.y_label, |lbl| ax.set_y_label(lbl, &[]));
    match options.legenc_loc {
        LegendLoc::None => {}
        LegendLoc::TopLeft => {
            ax.set_legend(Graph(0.0), Graph(0.95), &[Placement(AlignLeft, AlignTop)], &[]);
        }
        LegendLoc::BottomRight => {
            ax.set_legend(Graph(1.0), Graph(0.05), &[Placement(AlignRight, AlignBottom)], &[]);
        }
    }
    when(&options.min_x, |&min_x| ax.set_x_range(Fix(min_x), Auto));

    if let Some(file) = options.file.as_ref() {
        let (dim_x, dim_y) = options.dimensions;
        let file = Path::new(&options.out_dir).join(format!("{file}.pdf"));
        println!("Writing plot to {file:?}");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        fg.save_to_pdf(&file, dim_x, dim_y).unwrap();
    } else {
        fg.show().unwrap();
    }
}
