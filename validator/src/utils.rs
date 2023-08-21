use anyhow::{ensure, Result};
use regex::Regex;

/// Extracts the bounds stored in the input.
///
/// For example, extracts `(0, 100)` from `'integer[0, 100]'`.
pub fn extract_bounds(input: &str) -> Result<Option<(i64, i64)>> {
    let re = Regex::new(r#"\[(\d+), (\d+)\]"#).unwrap();
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
        assert_eq!(extract_bounds("integer[0, 100]").unwrap().unwrap(), (0, 100));
        assert_eq!(extract_bounds("integer[50, 70]").unwrap().unwrap(), (50, 70));
        assert_eq!(extract_bounds("real[0, 100]").unwrap().unwrap(), (0, 100));
        assert_eq!(extract_bounds("real[50, 70]").unwrap().unwrap(), (50, 70));
        assert_eq!(extract_bounds("foo[0, 100]").unwrap().unwrap(), (0, 100));
        assert_eq!(extract_bounds("foo[50, 70]").unwrap().unwrap(), (50, 70));

        assert!(extract_bounds("integer[100, 0]").is_err());
        assert!(extract_bounds("integer[70, 50]").is_err());
        assert!(extract_bounds("real[100, 0]").is_err());
        assert!(extract_bounds("real[70, 50]").is_err());
        assert!(extract_bounds("foo[100, 0]").is_err());
        assert!(extract_bounds("foo[70, 50]").is_err());

        assert!(extract_bounds("integer").unwrap().is_none());
        assert!(extract_bounds("real").unwrap().is_none());
        assert!(extract_bounds("foo").unwrap().is_none());
    }
}
