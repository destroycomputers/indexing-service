//! This module defines building blocks for the index storage.
mod avl;
mod avl_storage;

use std::path::PathBuf;

pub(crate) use avl::{Avl, MvccAvl};
pub(crate) use avl_storage::AvlStorage;

use crate::intern::InternRef;

/// Index entry.
///
/// For the given term, a list of index entries is associated, that stores
/// what files and at what offset contain the given term.
#[derive(Clone)]
pub(crate) struct IndexEntry {
    pub path: InternRef<PathBuf>,
    pub offset: u64,
}

/// List of index entries.
#[derive(Clone)]
pub(crate) struct IndexEntryList {
    // Key is a fake key for the AVL. We want to store a list of values and using AVL for this
    // only to limit the depth of recursion required to drop the list.
    key: usize,
    avl: Avl<usize, IndexEntry>,
}

impl IndexEntryList {
    pub fn new() -> Self {
        Self {
            key: 0,
            avl: Avl::new(),
        }
    }

    pub fn append(&self, entry: IndexEntry) -> Self {
        Self {
            key: self.key + 1,
            avl: self.avl.insert(self.key, entry),
        }
    }

    pub fn iter(&self) -> avl::Iter<'_, usize, IndexEntry> {
        self.avl.iter()
    }
}
