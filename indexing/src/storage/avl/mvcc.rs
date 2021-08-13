use std::{
    borrow::Borrow,
    sync::{Mutex, RwLock},
};

use super::Avl;

/// Mutable implementation of the AVL tree.
///
/// This is a wrapper around [`Avl`] that implements interior mutability.
///
/// Although, currently there's no actual versioning involved, this implements some basic
/// multi-version concurrency control scheme, in which reads don't block writes, while
/// writers apply changes using pessimistic write-lock.
///
/// Old versions of the tree obtained through [`MvccAvl::snapshot`] method will continue to be valid
/// after an update until all the references to them would be dropped.
///
/// The modifications are serialised, but through the duration of the modification itself the tree
/// is still accessible for taking snapshot. Only for a brief moment a write lock is issued to update
/// the tree root pointer.
///
/// To access the contents of the tree (get a value for a given key or iterater over the elements)
/// one must first create a snapshot of it by calling [`Mvcc::snapshot`]. The returned snapshot has
/// the necessary methods to access the values of the tree, see [`Avl`] and [`Avl::get`], [`Avl::iter`]
/// in particular.
pub struct MvccAvl<K, V> {
    root: RwLock<Avl<K, V>>,

    // This is only to serialise writers.
    write_lock: Mutex<()>,
}

impl<K, V> MvccAvl<K, V>
where
    K: Ord + Clone,
    V: Clone,
{
    /// Create a new instance of the AVL tree.
    pub fn new() -> Self {
        Self {
            root: RwLock::new(Avl::new()),
            write_lock: Mutex::new(()),
        }
    }

    /// Insert a new key-value pair in the tree.
    ///
    /// If the given key already exists in the tree, its associated value is updated with the newly supplied one.
    pub fn insert(&self, k: K, v: V) {
        let _write_lock = self.write_lock.lock();
        let new_root = self.snapshot().insert(k, v);

        *self.root.write().unwrap() = new_root;
    }

    /// Updates or inserts a new key-value pair in the tree.
    ///
    /// If the given key already exists in the tree, its current value is passed to the provided function,
    /// and the returned value will be the new associated with this key value. If the given key does not yet
    /// exist in the tree, a new node will be inserted and the provided function will be called with `None`
    /// to get an initial value to associate with this key.
    pub fn upsert<F>(&self, k: K, f: F)
    where
        F: FnOnce(Option<&V>) -> V,
    {
        let _write_lock = self.write_lock.lock();
        let new_root = self.snapshot().upsert(k, f);

        *self.root.write().unwrap() = new_root;
    }

    /// Updates an existing value in the tree.
    ///
    /// If the given key exists in the tree, its current value is passed to the provided function and the
    /// returned value will be the new associated with this key value.
    ///
    /// Otherwise, the function is never called and the tree is left unmodified.
    pub fn update<Q, F>(&self, k: &Q, f: F)
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
        F: FnOnce(&V) -> V,
    {
        let _write_lock = self.write_lock.lock();
        let new_root = self.snapshot().update(k, f);

        *self.root.write().unwrap() = new_root;
    }

    /// Remove the key-value pair associated with the given key from the tree.
    pub fn remove<Q>(&self, k: &Q)
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        let _write_lock = self.write_lock.lock();
        let new_root = self.snapshot().remove(k);

        *self.root.write().unwrap() = new_root;
    }

    /// Create a snapshot of the tree.
    pub fn snapshot(&self) -> Avl<K, V> {
        // Clone right away to drop the read lock.
        self.root.read().unwrap().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::MvccAvl;

    #[test]
    fn insert_updates_current_tree_snapshot() {
        let avl = MvccAvl::new();

        avl.insert("a", 1);

        assert_eq!(avl.snapshot().get("a").as_deref(), Some(&1));
    }

    #[test]
    fn update_updates_current_tree_snapshot() {
        let avl = MvccAvl::new();

        avl.insert("a", 1);
        avl.update("a", |v| v + 1);

        assert_eq!(avl.snapshot().get("a").as_deref(), Some(&2));
    }

    #[test]
    fn upsert_updates_current_tree_snapshot() {
        let avl = MvccAvl::new();

        avl.upsert("a", |_| 1);

        assert_eq!(avl.snapshot().get("a").as_deref(), Some(&1));
    }

    #[test]
    fn remove_updates_current_tree_snapshot() {
        let avl = MvccAvl::new();

        avl.insert("a", 1);
        avl.remove("a");

        assert_eq!(avl.snapshot().get("a").as_deref(), None);
    }
}
