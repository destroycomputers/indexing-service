use std::path::{Path, PathBuf};

use crate::{
    intern::InternPool,
    storage::{
        avl::{AvlSet, MvccAvl, ValueRef},
        IndexEntryList,
    },
    tokenise::Token,
};

/// Index storage that uses [`Avl`] as a data container.
pub(crate) struct AvlStorage {
    intern_pool: InternPool<PathBuf>,
    avl: MvccAvl<String, IndexEntryList>,
    file_words: MvccAvl<PathBuf, AvlSet<String>>,
}

impl AvlStorage {
    /// Create an instance of [`AvlStorage`].
    pub fn new() -> Self {
        Self {
            intern_pool: InternPool::new(),
            avl: MvccAvl::new(),
            file_words: MvccAvl::new(),
        }
    }

    /// Get a list of [`IndexEntry`] instances associated with this term (if any).
    pub fn get(&self, word: &str) -> Option<ValueRef<String, IndexEntryList>> {
        self.avl.snapshot().get(word)
    }

    /// Purge the given `path` from the index.
    pub fn purge(&self, path: &Path) {
        let interned_path = self.intern_pool.intern(path);

        let words = match self.file_words.snapshot().get(path) {
            Some(words) => words,
            None => return,
        };
        self.file_words.remove(path);

        for (word, _) in words.iter() {
            self.avl.update(word, |e| e.remove(&interned_path));
        }
    }

    /// Insert an token-path association in the index.
    pub fn insert(&self, path: &Path, token: Token) {
        let Token { value, offset } = token;

        self.file_words.upsert(path.to_owned(), |set| {
            set.as_deref()
                .cloned()
                .unwrap_or_else(AvlSet::new)
                .insert(value.clone(), ())
        });

        self.avl.upsert(value, |entries| {
            let entries = entries.cloned().unwrap_or_else(IndexEntryList::new);

            entries.append(self.intern_pool.intern(path), offset)
        })
    }
}
