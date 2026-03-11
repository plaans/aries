use std::path::Path;

use derive_builder::Builder;
use itertools::Itertools;

use crate::aggregator::Aggregator;
use crate::metric::Metric;
use crate::results::ResultCollection;
use crate::*;

pub type Selector = dyn Fn(&Problem, &SolverID) -> String;

#[derive(Clone, Builder)]
#[builder(pattern = "owned")]
#[builder(setter(strip_option))]
#[builder(derive(Clone))]
pub struct TableOptions {
    #[builder(default = "true")]
    pub highlight_best: bool,
    #[builder(default = "\"/tmp/tables\".to_string()")]
    pub out_dir: String,
    pub file: Option<String>,
}

pub fn print_latex_table<M: Metric + 'static, Agg: Aggregator<M::T>>(
    results: &ResultCollection,
    raw: &Selector,
    col: &Selector,
    metric: M,
    agg: Agg,
    fmt: impl Fn(M::T) -> String,
    options: TableOptions,
) where
    M::T: Copy + PartialOrd,
{
    let measures = results.measures(metric);
    let res = agg.aggregate(measures, |(pb, solver, m)| ((raw(pb, solver), (col(pb, solver))), m));
    let raws = res.keys().map(|(r, _)| r).sorted().dedup().collect_vec();
    let cols = res.keys().map(|(_, c)| c).sorted().dedup().collect_vec();

    let is_best = |raw: String, col: String| -> bool {
        // best in the minimu in col
        let best = raws
            .iter()
            .filter_map(|r| res.get(&(r.to_string(), col.to_string())))
            .max_by(|a, b| metric.compare(**a, **b));
        let cur = res.get(&(raw, col)).unwrap();
        if let Some(best) = best
            && metric.compare(*best, *cur).is_eq()
            && options.highlight_best
        {
            true
        } else {
            false
        }
    };

    use std::fmt::Write;
    let mut out = String::new();

    writeln!(out, "\\begin{{tabular}}{{l{}}}", "r".repeat(cols.len())).unwrap();
    for c in &cols {
        write!(out, " & \\colname{{{}}}", c).unwrap();
    }
    writeln!(out, " \\\\").unwrap();
    for r in &raws {
        write!(out, "\\rowname{{{r}}}").unwrap();
        for c in &cols {
            let key = ((*r).clone(), (*c).clone());
            if let Some(val) = res.get(&key).copied() {
                let best = is_best(r.to_string(), c.to_string());
                let value = if best {
                    format!("\\best{{{}}}", fmt(val))
                } else {
                    format!("     {} ", fmt(val))
                };
                write!(out, " & {:>15}", value).unwrap();
                // if is_best(r.to_string(), c.to_string()) {
                //     write!(out, "*").unwrap();
                // } else {
                //     write!(out, " ").unwrap();
                // }
                // print!("\t{:>10}", val);
            }
        }
        writeln!(out, " \\\\").unwrap();
    }
    writeln!(out, "\\end{{tabular}}").unwrap();

    if let Some(file) = options.file.as_ref() {
        std::fs::create_dir_all(&options.out_dir).unwrap();
        let file = Path::new(&options.out_dir).join(file);
        println!("Writing table to {file:?}");
        std::fs::write(file, out).unwrap();
    } else {
        println!("{out}")
    }
}
