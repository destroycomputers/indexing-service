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
mod storage;

use std::{
    collections::HashSet,
    path::Path,
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

use notify::{self, DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use tracing::{error, info, instrument, trace, warn};
use walkdir::WalkDir;

pub use error::{Error, Result};
pub use indexer::Indexer;

/// LiveIndexer is a wrapper around [`Indexer`] which automatically manages the index for the watched paths.
///
/// It can be configured to watch certain directories or files for changes and reevaluating the index
/// for those paths (adding newly created files to the index, removing deleted files from the index or
/// updating the index of modified files).
///
/// Instances of `LiveIndexer` can be created with
pub struct LiveIndexer {
    indexer: Arc<Indexer>,
    watcher: Mutex<RecommendedWatcher>,
}

impl LiveIndexer {
    /// Start the live indexer.
    ///
    /// This sets up the file watcher, so that new paths can be watched by invoking [`LiveIndexer::watch`] method.
    ///
    /// The returned value is `self` wrapped in an [`std::sync::Arc`] that can be safely accessed from different threads.
    pub fn start(indexer: Indexer) -> Result<Arc<Self>> {
        let (tx, watcher_event_rx) = mpsc::channel();

        let live_indexer = Arc::new(Self {
            indexer: Arc::new(indexer),
            watcher: Mutex::new(notify::watcher(tx, Duration::from_secs(1))?),
        });
        let live_indexer_ref = Arc::clone(&live_indexer);

        thread::spawn(move || loop {
            let event = match watcher_event_rx.recv() {
                Ok(event) => event,
                Err(_) => {
                    info!("file watcher is shutting down");
                    break;
                }
            };

            if let Err(e) = live_indexer.process_watcher_event(event) {
                warn!(error = %e, "file watcher has encountered an error");
            }
        });

        Ok(live_indexer_ref)
    }

    /// Build an index for the given path and watch it for changes.
    #[instrument(skip(self, path), fields(path = %path.as_ref().display()))]
    pub fn watch<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        info!("watching a new path");

        for entry in WalkDir::new(path.as_ref().canonicalize()?) {
            let entry = entry?;

            if let Err(e) = self.indexer.index_file(entry.path()) {
                warn!(error = %e, "failed to index a file");
            }
        }

        self.watcher
            .lock()
            .unwrap()
            .watch(path, RecursiveMode::Recursive)
            .map_err(Into::into)
    }

    /// Remove a previously set watcher and the given path from the index.
    #[instrument(skip(self, path), fields(path = %path.as_ref().display()))]
    pub fn unwatch<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        info!("unwatching a path");

        for entry in WalkDir::new(path.as_ref().canonicalize()?) {
            let entry = entry?;

            self.indexer.clear_from_index(entry.path());
        }

        self.watcher
            .lock()
            .unwrap()
            .unwatch(path)
            .map_err(Into::into)
    }

    /// Passes the query down to the [`Indexer`] returning the set of file paths that got a hit for the
    /// given term.
    ///
    /// See [`Indexer::query`] for more information.
    pub fn query(&self, term: &str) -> HashSet<String> {
        self.indexer.query(term)
    }

    /// Process the given watch event.
    fn process_watcher_event(&self, event: notify::DebouncedEvent) -> Result<()> {
        match event {
            DebouncedEvent::Write(path) => {
                trace!(path = %path.display(), "file write event");

                self.indexer.clear_from_index(&path);
                self.indexer.index_file(&path)
            }

            DebouncedEvent::Create(path) => self.indexer.index_file(&path),

            DebouncedEvent::Remove(path) => Ok(self.indexer.clear_from_index(&path)),

            DebouncedEvent::Rename(path_old, path_new) => {
                trace!(old = %path_old.display(), new = %path_new.display(), "file rename event");

                self.indexer.clear_from_index(&path_old);
                self.indexer.index_file(&path_new)
            }

            DebouncedEvent::Error(e, p) => {
                error!(error = %e, path = ?p.as_ref().map(|p| p.display()), "watcher sent an error");
                Ok(())
            }

            // These events are ignored. They could be useful for additional robustness in the future.
            DebouncedEvent::Rescan => Ok(()),
            DebouncedEvent::Chmod(_) => Ok(()),
            DebouncedEvent::NoticeWrite(_) => Ok(()),
            DebouncedEvent::NoticeRemove(_) => Ok(()),
        }
    }
}
