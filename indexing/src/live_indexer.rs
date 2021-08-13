use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

use notify::{self, DebouncedEvent, RecommendedWatcher, RecursiveMode, Watcher};
use tracing::{error, info, instrument, trace, warn};
use walkdir::WalkDir;

use crate::{Indexer, Result};

/// LiveIndexer is a wrapper around [`Indexer`] which automatically manages the index for the watched paths.
///
/// It can be configured to watch certain directories or files for changes and reevaluating the index
/// for those paths (adding newly created files to the index, removing deleted files from the index or
/// updating the index of modified files).
///
/// Instances of `LiveIndexer` can be created with
pub struct LiveIndexer {
    indexer: Arc<Indexer>,
    indexing_queue: mpsc::Sender<IndexingAction>,
    watcher: Mutex<RecommendedWatcher>,
}

impl LiveIndexer {
    /// Start the live indexer.
    ///
    /// This sets up the file watcher, so that new paths can be watched by invoking [`LiveIndexer::watch`] method.
    ///
    /// The returned value is `self` wrapped in an [`std::sync::Arc`] that can be safely accessed from different threads.
    pub fn start(indexer: Indexer) -> Result<Self> {
        let (tx, watcher_event_rx) = mpsc::channel();
        let indexer = Arc::new(indexer);

        let indexing_queue = spawn_indexing_worker(Arc::clone(&indexer));
        spawn_watching_worker(indexing_queue.clone(), watcher_event_rx);

        Ok(Self {
            indexer,
            indexing_queue,
            watcher: Mutex::new(notify::watcher(tx, Duration::from_secs(1))?),
        })
    }

    /// Build an index for the given path and watch it for changes.
    #[instrument(skip(self, path), fields(path = %path.as_ref().display()))]
    pub fn watch<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        info!("watching a new path");

        let path = path.as_ref();

        self.watcher
            .lock()
            .unwrap()
            .watch(path, RecursiveMode::Recursive)?;
        self.indexing_queue
            .send(IndexingAction::AddDir {
                path: path.to_owned(),
            })
            .unwrap();

        Ok(())
    }

    /// Remove a previously set watcher and the given path from the index.
    #[instrument(skip(self, path), fields(path = %path.as_ref().display()))]
    pub fn unwatch<P>(&self, path: P) -> Result<()>
    where
        P: AsRef<Path>,
    {
        info!("unwatching a path");

        let path = path.as_ref();

        self.watcher.lock().unwrap().unwatch(path)?;
        self.indexing_queue
            .send(IndexingAction::RemoveDir {
                path: path.to_owned(),
            })
            .unwrap();

        Ok(())
    }

    /// Passes the query down to the [`Indexer`] returning the set of file paths that got a hit for the
    /// given term.
    ///
    /// See [`Indexer::query`] for more information.
    pub fn query(&self, term: &str) -> HashSet<String> {
        self.indexer.query(term)
    }
}

/// Action to be performed by indexing worker.
///
/// See [`spawn_indexing_worker`].
enum IndexingAction {
    Add { path: PathBuf },
    AddDir { path: PathBuf },
    Remove { path: PathBuf },
    RemoveDir { path: PathBuf },
}

/// Spawn an indexing worker.
///
/// This worker performs mutating indexing operations on the index (index/clear) in a separate thread.
///
/// Returns an [`mpsc::Sender`] that allows to enqueue tasks for this worker.
///
/// NOTE: since the only normal condition for this worker to shutdown is when all the senders
/// are dropped, it is safe to `.unwrap()` sends on the returned by this function sender.
fn spawn_indexing_worker(indexer: Arc<Indexer>) -> mpsc::Sender<IndexingAction> {
    fn add_dir(indexer: &Indexer, path: &Path) -> Result<()> {
        for entry in WalkDir::new(path.canonicalize()?) {
            let entry = entry?;

            if let Err(e) = indexer.index_file(entry.path()) {
                warn!(error = %e, "failed to index a file");
            }
        }
        Ok(())
    }

    fn remove_dir(indexer: &Indexer, path: &Path) -> Result<()> {
        for entry in WalkDir::new(path.canonicalize()?) {
            let entry = entry?;

            indexer.clear_from_index(entry.path());
        }
        Ok(())
    }

    let (tx, indexing_queue_rx) = mpsc::channel();

    thread::spawn(move || {
        while let Ok(action) = indexing_queue_rx.recv() {
            let r = match action {
                IndexingAction::Add { path } => indexer.index_file(&path),
                IndexingAction::AddDir { path } => add_dir(&indexer, &path),
                IndexingAction::Remove { path } => Ok(indexer.clear_from_index(&path)),
                IndexingAction::RemoveDir { path } => remove_dir(&indexer, &path),
            };

            if let Err(e) = r {
                warn!(error = %e, "indexing error");
            }
        }
    });

    tx
}

/// Spawn filesystem watching worker.
///
/// This worker listens for file events in a separate thread and queues corresponding [`IndexingAction`]s
/// to the indexing worker.
fn spawn_watching_worker(
    indexing_queue: mpsc::Sender<IndexingAction>,
    watcher_event_rx: mpsc::Receiver<notify::DebouncedEvent>,
) {
    thread::spawn(move || {
        while let Ok(event) = watcher_event_rx.recv() {
            match event {
                DebouncedEvent::Write(path) => {
                    trace!(path = %path.display(), "file write event");

                    indexing_queue
                        .send(IndexingAction::Remove { path: path.clone() })
                        .unwrap();
                    indexing_queue.send(IndexingAction::Add { path }).unwrap();
                }

                DebouncedEvent::Create(path) => {
                    trace!(path = %path.display(), "file create event");

                    indexing_queue.send(IndexingAction::Add { path }).unwrap();
                }

                DebouncedEvent::Remove(path) => {
                    trace!(path = %path.display(), "file remove event");

                    indexing_queue
                        .send(IndexingAction::Remove { path })
                        .unwrap();
                }

                DebouncedEvent::Rename(path_old, path_new) => {
                    trace!(old = %path_old.display(), new = %path_new.display(), "file rename event");

                    indexing_queue
                        .send(IndexingAction::Remove { path: path_old })
                        .unwrap();
                    indexing_queue
                        .send(IndexingAction::Add { path: path_new })
                        .unwrap();
                }

                DebouncedEvent::Error(e, p) => {
                    error!(error = %e, path = ?p.as_ref().map(|p| p.display()), "watcher sent an error");
                }

                // These events are ignored. They could be useful for additional robustness in the future.
                DebouncedEvent::Rescan => (),
                DebouncedEvent::Chmod(_) => (),
                DebouncedEvent::NoticeWrite(_) => (),
                DebouncedEvent::NoticeRemove(_) => (),
            };
        }

        info!("file watcher is shutting down");
    });
}
