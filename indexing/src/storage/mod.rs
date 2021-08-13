//! This module defines building blocks for the index storage.
mod avl;
mod avl_storage;
mod list;

use std::path::PathBuf;

pub(crate) use avl::MvccAvl;
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
