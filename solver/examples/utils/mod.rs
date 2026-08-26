use std::{collections::VecDeque, str::FromStr};

/// Simplistic parser to help reading problem definition.
///
/// Note: will panic anytime something is not exactly as expected.
pub struct Parser<'a> {
    words: VecDeque<&'a str>,
}

#[allow(unused)]
impl<'a> Parser<'a> {
    pub fn new(input: &'a str) -> Self {
        Self {
            words: input.split_whitespace().collect(),
        }
    }

    /// Returns true if there is no words left in the input.
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// Parse the next word into a given type.
    pub fn pop<T: FromStr>(&mut self) -> T {
        self.words
            .pop_front()
            .expect("nothing to read")
            .parse()
            .ok()
            .expect("parse error")
    }

    /// Remove the next word, checking that it contains the given value.
    pub fn ignore_expected<T: FromStr + Eq>(&mut self, expected: T) {
        let read: T = self.pop();
        assert!(read == expected);
    }

    /// Remove words until the given value is found
    pub fn ignore_until<T: FromStr + Eq>(&mut self, expected: T) {
        let mut read: T = self.pop();
        while read != expected {
            read = self.pop();
        }
    }

    /// Remove words until the given value plus the following double dot (with or without a whitespace in between)
    pub fn ignore_until_double_dot(&mut self, expected: String) {
        let expected_dot = format!("{expected}:");
        let mut read: String = self.pop();
        loop {
            if read == expected {
                self.ignore_expected(String::from(":"));
                break;
            } else if read == expected_dot {
                break;
            }

            read = self.pop();
        }
    }
}
