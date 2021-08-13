//! This library provides facilities to perform and maintain an in-memory text index.
//!
//! The index can be used to query if an entry exists in any of the indexed files and
//! returns the list of files it was found it.
//!
//! Indexing is performed by splitting the provided text file in tokens and building
//! an index tree that allows for fast queries. Tokenisation is facilitated by tokenisers
//! (see [`tokenise`] module documentaiton) and normalisers (see [`normalise`] module documentaiton).
//!
//! The index can be automatically maintained by the means of [`LiveIndexer`] which
//! watches the files and performs an indexing/purging as a reaction on watch events.

pub mod normalise;
pub mod tokenise;

mod error;
mod indexer;
mod intern;
mod live_indexer;
mod storage;

pub use error::{Error, Result};
pub use indexer::Indexer;
pub use live_indexer::LiveIndexer;
