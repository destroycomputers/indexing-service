use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    intern::InternPool,
    storage::{
        avl::{Avl, ValueRef},
        list::List,
        IndexEntry,
    },
    tokenise::Token,
};

/// Index storage that uses [`Avl`] as a data container.
pub(crate) struct AvlStorage {
    intern_pool: InternPool<PathBuf>,
    avl: Avl<String, Arc<List<IndexEntry>>>,
}

impl AvlStorage {
    /// Create an instance of [`AvlStorage`].
    pub fn new() -> Self {
        Self {
            intern_pool: InternPool::new(),
            avl: Avl::new(),
        }
    }

    /// Get a list of [`IndexEntry`] instances associated with this term (if any).
    pub fn get(&self, word: &str) -> Option<ValueRef<String, Arc<List<IndexEntry>>>> {
        self.avl.get(word)
    }

    /// Purge the given `path` from the index.
    pub fn purge(&self, path: &Path) {
        let interned_path = self.intern_pool.intern(path);
        for (k, v) in self.avl.iter() {
            if v.iter().any(|entry| entry.path == interned_path) {
                self.avl.update(k, |e| {
                    e.iter().fold(Arc::new(List::Null), |l, e| {
                        if e.path == interned_path {
                            l
                        } else {
                            Arc::new(List::Cons(e.clone(), l))
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
            Arc::new(List::Cons(
                IndexEntry {
                    path: self.intern_pool.intern(path),
                    offset,
                },
                entries.cloned().unwrap_or_else(|| Arc::new(List::Null)),
            ))
        })
    }
}
