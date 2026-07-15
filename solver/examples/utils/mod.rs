use std::{collections::VecDeque, str::FromStr};

/// Simplistic parser to help reading a problem definition.
///
/// Note: will panic anytime something is not exactly as expected.
pub struct Parser<'a> {
    words: VecDeque<&'a str>,
}

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
}
