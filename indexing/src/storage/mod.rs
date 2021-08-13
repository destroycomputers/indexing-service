//! This module defines building blocks for the index storage.
mod avl;
mod avl_storage;

use std::path::PathBuf;

pub(crate) use avl::{Avl, AvlSet, MvccAvl};
pub(crate) use avl_storage::AvlStorage;

use crate::intern::InternRef;

#[derive(Clone)]
pub(crate) struct IndexEntryList {
    pub entries: Avl<InternRef<PathBuf>, AvlSet<u64>>,
}

impl IndexEntryList {
    pub fn new() -> Self {
        Self {
            entries: Avl::new(),
        }
    }

    pub fn append(&self, path: InternRef<PathBuf>, offset: u64) -> Self {
        Self {
            entries: self.entries.upsert(path, |set| {
                set.as_deref()
                    .cloned()
                    .unwrap_or_else(AvlSet::new)
                    .insert(offset, ())
            }),
        }
    }

    pub fn remove(&self, path: &InternRef<PathBuf>) -> Self {
        Self {
            entries: self.entries.remove(path),
        }
    }

    pub fn iter(&self) -> avl::Iter<'_, InternRef<PathBuf>, AvlSet<u64>> {
        self.entries.iter()
    }
}
