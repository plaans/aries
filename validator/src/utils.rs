use anyhow::{ensure, Result};
use regex::Regex;

/// Extracts the bounds stored in the input.
///
/// For example, extracts `(0, 100)` from `'integer[0, 100]'`.
pub fn extract_bounds(input: &str) -> Result<Option<(i64, i64)>> {
    let re = Regex::new(r#"\[(-?\d+), (-?\d+)\]"#).unwrap();
    if let Some(captures) = re.captures(input) {
        let lower: i64 = captures[1].parse().unwrap();
        let upper: i64 = captures[2].parse().unwrap();
        ensure!(lower <= upper);
        Ok(Some((lower, upper)))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_bounds() {
        for name in ["integer", "real", "foo", "bar"] {
            let empty_res = extract_bounds(name.to_string().as_str());
            assert!(empty_res.is_ok(), "{}", name);
            assert!(empty_res.unwrap().is_none(), "{}", name);

            for lb_f in [-1, 1] {
                for lb in [0, 1, 2, 5, 10, 12, 15, 20, 50, 100] {
                    for ub_f in [-1, 1] {
                        for ub in [0, 1, 2, 5, 10, 12, 15, 20, 50, 100] {
                            let lb_v = lb_f * lb;
                            let ub_v = ub_f * ub;
                            let input = format!("{name}[{lb_v}, {ub_v}]");
                            let is_ok = lb_v <= ub_v;
                            let res = extract_bounds(input.as_str());

                            assert_eq!(res.is_ok(), is_ok, "{}", input);
                            if is_ok {
                                assert!(res.as_ref().unwrap().is_some(), "{}", input);
                                assert_eq!(res.unwrap().unwrap(), (lb_v, ub_v), "{}", input);
                            }
                        }
                    }
                }
            }
        }
    }
}
