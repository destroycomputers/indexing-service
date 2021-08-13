use std::path::{Path, PathBuf};

use crate::{
    intern::InternPool,
    storage::{
        avl::{MvccAvl, ValueRef},
        IndexEntry, IndexEntryList,
    },
    tokenise::Token,
};

/// Index storage that uses [`Avl`] as a data container.
pub(crate) struct AvlStorage {
    intern_pool: InternPool<PathBuf>,
    avl: MvccAvl<String, IndexEntryList>,
}

impl AvlStorage {
    /// Create an instance of [`AvlStorage`].
    pub fn new() -> Self {
        Self {
            intern_pool: InternPool::new(),
            avl: MvccAvl::new(),
        }
    }

    /// Get a list of [`IndexEntry`] instances associated with this term (if any).
    pub fn get(&self, word: &str) -> Option<ValueRef<String, IndexEntryList>> {
        self.avl.snapshot().get(word)
    }

    /// Purge the given `path` from the index.
    pub fn purge(&self, path: &Path) {
        let interned_path = self.intern_pool.intern(path);
        for (k, v) in self.avl.snapshot().iter() {
            if v.iter().any(|(_, entry)| entry.path == interned_path) {
                self.avl.update(k, |e| {
                    e.iter().fold(IndexEntryList::new(), |l, (_, e)| {
                        if e.path == interned_path {
                            l
                        } else {
                            l.append(e.clone())
                        }
                    })
                });
            }
        }
    }

    /// Insert an token-path association in the index.
    pub fn insert(&self, path: &Path, token: Token) {
        let Token { value, offset } = token;

        self.avl.upsert(value, |entries| {
            let entries = entries.cloned().unwrap_or_else(IndexEntryList::new);

            entries.append(IndexEntry {
                path: self.intern_pool.intern(path),
                offset,
            })
        })
    }
}
