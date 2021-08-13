use std::io;

use thiserror::Error;

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Error that can be returned by [`crate::indexer::Indexer`] methods.
#[derive(Debug, Error)]
pub enum Error {
    /// File watcher errors.
    #[error(transparent)]
    Notify(#[from] notify::Error),

    /// I/O errors.
    #[error(transparent)]
    Io(#[from] io::Error),

    /// Walkdir errors.
    #[error(transparent)]
    WalkDir(#[from] walkdir::Error),
}
