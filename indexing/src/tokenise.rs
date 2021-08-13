//! This module defines tokenising facilities to split the given text into tokens.
//!
//! Tokeniser is any type that implements [`Tokeniser`] trait. There are several predefined tokenisers:
//!  * `SpaceTokeniser` - splits input by white space
//!  * `RegexTokeniser` - splits input by the provided regex
//!
//! To use a [`Tokeniser`] with an [`crate::indexer::Indexer`] one should implement a [`TokeniserFactory`]
//! that creates a fresh tokeniser instance in a read-to-use state. For the simple case, [`TokeniserFactory`]
//! is implemented on `Fn() -> Box<dyn Tokeniser>`.
use std::{
    hash::Hash,
    io::{self, BufRead},
    slice, str,
};

use regex::Regex;

/// Token specifies a parsed value and its original offset in the file.
///
/// The length of the value and its representation may differ from the original found in the file due
/// to applied normalisers.
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Token {
    /// Token's string value.
    pub value: String,

    /// Byte offset in the source text.
    pub offset: u64,
}

impl Token {
    /// Create a new token with the given string value and an offset of zero.
    pub fn new(value: String) -> Self {
        Self { value, offset: 0 }
    }

    /// Create a new token with the given string value at the specified offset.
    pub fn with_offset_at(value: String, offset: u64) -> Self {
        Self { value, offset }
    }
}

/// Produces [`Tokeniser`]s in a ready-to-use state.
///
/// Any `Fn() -> Box<dyn Tokeniser>` implements this trait.
pub trait TokeniserFactory: Send + Sync {
    fn create(&self) -> Box<dyn Tokeniser>;
}

impl<F> TokeniserFactory for F
where
    F: Send + Sync + Fn() -> Box<dyn Tokeniser>,
{
    fn create(&self) -> Box<dyn Tokeniser> {
        self()
    }
}

/// Tokeniser knows how to split an incoming text in distinct tokens.
pub trait Tokeniser: Send + Sync {
    /// Read a subsequent token from the given `reader`.
    ///
    /// Reader should not be used between the calls to this function.
    ///
    /// The underlying implementation  doesn't have to read data from the `reader` on every call,
    /// it may buffer the input and return subsequent tokens without performing additional reads.
    ///
    /// The returned value `Ok(None)` signifies the end of the token stream.
    fn read_token(&mut self, reader: &mut dyn BufRead) -> io::Result<Option<Token>>;
}

/// Tokeniser that splits the input into tokens by white space.
#[derive(Clone)]
pub struct SpaceTokeniser {
    input: String,
    words: Vec<(*const u8, usize)>,
    given: usize,
}

unsafe impl Send for SpaceTokeniser {}
unsafe impl Sync for SpaceTokeniser {}

impl SpaceTokeniser {
    pub fn new() -> Self {
        Self {
            input: String::new(),
            words: Vec::new(),
            given: 0,
        }
    }
}

impl Tokeniser for SpaceTokeniser {
    fn read_token(&mut self, reader: &mut dyn BufRead) -> io::Result<Option<Token>> {
        // NOTE: generally one would implement incremental reading from the `reader`.
        // I didn't have time for a proper implementation like that, so here I reuse
        // `split_whitespace` method on strings and simply read the whole input once.
        if self.given == 0 {
            reader.read_to_string(&mut self.input)?;

            self.words
                .extend(self.input.split_whitespace().map(|s| (s.as_ptr(), s.len())))
        }

        if self.given == self.words.len() {
            Ok(None)
        } else {
            let (word_ptr, word_len) = self.words[self.given];
            // Don't judge me.
            let token = unsafe {
                Token {
                    // We don't have to check for UTF-8 correctness as this is a view into a `String`
                    // that was already verified to be UTF-8 correct.
                    value: str::from_utf8_unchecked(slice::from_raw_parts(word_ptr, word_len))
                        .to_owned(),
                    offset: word_ptr.offset_from(self.input.as_ptr()) as u64,
                }
            };
            self.given += 1;
            Ok(Some(token))
        }
    }
}

/// Tokeniser that splits the input into tokens by the provided regex.
#[derive(Clone)]
pub struct RegexTokeniser {
    input: String,
    words: Vec<(*const u8, usize)>,
    given: usize,
    regex: Regex,
}

unsafe impl Send for RegexTokeniser {}
unsafe impl Sync for RegexTokeniser {}

impl RegexTokeniser {
    pub fn new(pattern: &str) -> Result<Self, regex::Error> {
        Ok(Self {
            input: String::new(),
            words: Vec::new(),
            given: 0,
            regex: Regex::new(pattern)?,
        })
    }
}

impl Tokeniser for RegexTokeniser {
    fn read_token(&mut self, reader: &mut dyn BufRead) -> io::Result<Option<Token>> {
        // NOTE: generally one would implement incremental reading from the `reader`.
        // I didn't have time for a proper implementation like that, so here I reuse
        // `split_whitespace` method on strings and simply read the whole input once.
        if self.given == 0 {
            reader.read_to_string(&mut self.input)?;

            self.words
                .extend(self.regex.split(&self.input).map(|s| (s.as_ptr(), s.len())));
        }

        if self.given == self.words.len() {
            Ok(None)
        } else {
            let (word_ptr, word_len) = self.words[self.given];
            let token = unsafe {
                Token {
                    // We don't have to check for UTF-8 correctness as this is a view into a `String`
                    // that was already verified to be UTF-8 correct.
                    value: str::from_utf8_unchecked(slice::from_raw_parts(word_ptr, word_len))
                        .to_owned(),
                    offset: word_ptr.offset_from(self.input.as_ptr()) as u64,
                }
            };
            self.given += 1;
            Ok(Some(token))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn token(value: &str, offset: u64) -> Token {
        Token::with_offset_at(value.to_owned(), offset)
    }

    #[test]
    fn word_tokeniser_splits_by_whitespace() {
        let input = "one\ntwo    three";
        let mut tokeniser = SpaceTokeniser::new();
        let mut reader = input.as_bytes();

        assert_eq!(
            tokeniser.read_token(&mut reader).unwrap(),
            Some(token("one", 0))
        );
        assert_eq!(
            tokeniser.read_token(&mut reader).unwrap(),
            Some(token("two", 4))
        );
        assert_eq!(
            tokeniser.read_token(&mut reader).unwrap(),
            Some(token("three", 11))
        );
        assert_eq!(tokeniser.read_token(&mut reader).unwrap(), None);
    }

    #[test]
    fn regex_tokeniser_splits_by_regex() {
        let input = "one, two\n[] three";
        let mut tokeniser = RegexTokeniser::new(r"\W+").unwrap();
        let mut reader = input.as_bytes();

        assert_eq!(
            tokeniser.read_token(&mut reader).unwrap(),
            Some(token("one", 0))
        );
        assert_eq!(
            tokeniser.read_token(&mut reader).unwrap(),
            Some(token("two", 5))
        );
        assert_eq!(
            tokeniser.read_token(&mut reader).unwrap(),
            Some(token("three", 12))
        );
        assert_eq!(tokeniser.read_token(&mut reader).unwrap(), None);
    }
}
