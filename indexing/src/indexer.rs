use std::{collections::HashSet, fs, io::BufReader, path::Path, time::Instant};

use tracing::{instrument, trace};

use crate::{normalise, storage::AvlStorage, tokenise, Result};

/// Indexer builds a text index over the text files under the provided paths.
///
/// The built index can be queried with a specific term to obtain the set of files that this term
/// was found in.
///
/// Indexing is performed by parsing a file using a type that implements [`tokenise::Tokeniser`],
/// created by the means of the provided [`tokenise::TokeniserFactory`]. There are several
/// predefined tokenisers, see [`tokenise`] module documentation for more information.
///
/// Tokens can be "normalised" by applying [`normalise::TokenNormaliser`] to them.
/// Several normalisers can be provided (see [`Indexer::with_normaliser`]), they will be applied
/// to every token in the order specified. There are several predefined normalisers, see [`normalise`]
/// module documentation for more information.
///
/// Indexer is thread-safe and can be used from several threads concurrenctly without additional
/// synchronisation required.
pub struct Indexer {
    storage: AvlStorage,
    tokeniser_factory: Box<dyn tokenise::TokeniserFactory>,
    token_normalisers: Vec<Box<dyn normalise::TokenNormaliser>>,
}

impl Indexer {
    /// Create a new [`Indexer`] with the provided [`tokenise::TokeniserFactory`].
    pub fn new<F>(tokeniser_factory: F) -> Self
    where
        F: 'static + tokenise::TokeniserFactory,
    {
        Self {
            storage: AvlStorage::new(),
            tokeniser_factory: Box::new(tokeniser_factory),
            token_normalisers: Vec::new(),
        }
    }

    /// Add a [`normalise::TokenNormaliser`] to be used by this [`Indexer`].
    pub fn with_normaliser<T>(mut self, normaliser: T) -> Self
    where
        T: 'static + normalise::TokenNormaliser,
    {
        self.token_normalisers.push(Box::new(normaliser));
        self
    }

    /// Query the index to find a set of files that the given term can be found in.
    ///
    /// The input is normalised the same way as the indexed files.
    pub fn query(&self, term: &str) -> HashSet<String> {
        let word = self
            .normalise(tokenise::Token::new(term.to_owned()))
            .map_or_else(|| term.to_owned(), |t| t.value);

        self.storage
            .get(&word)
            .map(|entries| {
                entries
                    .iter()
                    .map(|(_, entry)| entry.path.as_path())
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .map(|p| p.to_string_lossy().into_owned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Clear the given path from the index.
    ///
    /// Traverses an index and removes all the entries that refer to the given path.
    #[instrument(skip(self, path), fields(path = %path.display()))]
    pub fn clear_from_index(&self, path: &Path) {
        trace!("removing a file from index");
        self.storage.purge(path);
    }

    /// Add the given file to the index.
    ///
    /// `path` has to point to a file, otherwise the function returns without an error immediately.
    ///
    /// The input is canonicalised before processing. Pointed to file then parsed by the means of the
    /// supplied [`tokenise::Tokeniser`] and every token is normalised by the provided set of
    /// [`normalise::TokenNormaliser`]s before adding in the index.
    #[instrument(skip(self, path), fields(path = %path.display()))]
    pub fn index_file(&self, path: &Path) -> Result<()> {
        if !fs::metadata(path)?.file_type().is_file() {
            return Ok(());
        }

        let path = path.canonicalize()?;
        let mut reader = BufReader::new(fs::File::open(&path)?);
        let mut words_count = 0;
        let start = Instant::now();

        let mut tokeniser = self.tokeniser_factory.create();

        while let Some(token) = tokeniser.read_token(&mut reader)? {
            words_count += 1;

            if let Some(token) = self.normalise(token) {
                self.storage.insert(&path, token);
            }
        }

        trace!(duration = ?start.elapsed(), %words_count, "indexed a file");

        Ok(())
    }

    /// Normalise the given token by applying sequentially all configured normalisers.
    fn normalise(&self, token: tokenise::Token) -> Option<tokenise::Token> {
        self.token_normalisers
            .iter()
            .try_fold(token, |token, norm| norm.normalise(token))
    }
}
