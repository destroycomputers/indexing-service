//! This module defines a [`TokenNormaliser`] trait that facilitates token normalisation.
//!
//! There are several predefined normalisers:
//!  * [`StopWords`] - filters the tokens by the list of stop words
//!  * [`LowerCase`] - normalises tokens by converting them to lower case
//!  * [`Unicode`] - performs unicode normalisation of tokens
//!
//! Additionally, arbitrary normalisers can be defined by implementing [`TokenNormaliser`] trait.

use std::{collections::HashSet, ops::Not};

use unicode_normalization::UnicodeNormalization;

use super::tokenise::Token;

/// Token normaliser.
///
/// Normalisation is applied during file tokenisation and index querying.
///
/// See [`crate::indexer::Indexer`] documentation for how tokenisers can be used with an indexer.
pub trait TokenNormaliser: Send + Sync {
    fn normalise(&self, token: Token) -> Option<Token>;
}

/// Unicode normaliser.
///
/// Performs NFC, NFD, NFKC and NFKD unicode normalization as defined by the unicode standard.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Unicode {
    NFC,
    NFD,
    NFKC,
    NFKD,
}

impl TokenNormaliser for Unicode {
    fn normalise(&self, token: Token) -> Option<Token> {
        let value = match self {
            Unicode::NFC => token.value.nfc().collect(),
            Unicode::NFD => token.value.nfd().collect(),
            Unicode::NFKC => token.value.nfkc().collect(),
            Unicode::NFKD => token.value.nfkd().collect(),
        };

        Some(Token {
            value,
            offset: token.offset,
        })
    }
}

/// Stop word normaliser.
///
/// Filters out the tokens that match the specified list of stop words.
pub struct StopWords {
    stop_words: HashSet<String>,
}

impl StopWords {
    pub fn new(stop_words: &[&str]) -> Self {
        Self {
            stop_words: stop_words.iter().map(|&s| s.to_owned()).collect(),
        }
    }
}

impl TokenNormaliser for StopWords {
    fn normalise(&self, token: Token) -> Option<Token> {
        self.stop_words.contains(&token.value).not().then(|| token)
    }
}

/// Lower case normaliser.
///
/// Converts the token to the lower case.
pub struct LowerCase;

impl TokenNormaliser for LowerCase {
    fn normalise(&self, token: Token) -> Option<Token> {
        Some(Token {
            value: token.value.to_lowercase(),
            offset: token.offset,
        })
    }
}
