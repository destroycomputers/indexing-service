use std::{
    borrow::Borrow,
    cmp,
    ops::Deref,
    sync::{Arc, Mutex, RwLock},
};

/// AVL tree implementation.
///
/// This is a self-balancing tree which guarantees the difference in branches height to be no more than one.
/// Thus, the operations on the tree all have `O(log(N))` complexity.
///
/// It stores key-value pairs, with the condition that key implements `Ord` and both key and value are
/// cloneable.
///
/// The implementation uses interior mutability, it is safe to read and modify the tree from different
/// threads.
///
/// The modifications are serialised, but through the duration of the modification itself the tree
/// is still accessible for reading. Only for a brief moment a write lock is issued to update the tree
/// root pointer.
pub struct Avl<K, V> {
    root: RwLock<Option<Arc<Node<K, V>>>>,

    // This is only to serialise writers.
    write_lock: Mutex<()>,
}

impl<K, V> Avl<K, V>
where
    K: Ord + Clone,
    V: Clone,
{
    /// Create a new instance of the AVL tree.
    pub fn new() -> Self {
        Self {
            root: RwLock::new(None),
            write_lock: Mutex::new(()),
        }
    }

    /// Insert a new key-value pair in the tree.
    ///
    /// If the given key already exists in the tree, its associated value is updated with the newly supplied one.
    pub fn insert(&self, k: K, v: V) {
        let _write_lock = self.write_lock.lock();

        let new_root = if let Some(node) = self.snapshot() {
            Arc::new(node.upsert(k, |_| v))
        } else {
            Arc::new(Node::leaf(k, v))
        };

        *self.root.write().unwrap() = Some(new_root);
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

        let new_root = if let Some(node) = self.snapshot() {
            Arc::new(node.upsert(k, f))
        } else {
            Arc::new(Node::leaf(k, f(None)))
        };

        *self.root.write().unwrap() = Some(new_root);
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

        if let Some(node) = self.snapshot() {
            let new_root = node.update(k, f).map(Arc::new);

            *self.root.write().unwrap() = new_root;
        }
    }

    /// Remove the key-value pair associated with the given key from the tree.
    pub fn remove<Q>(&self, k: &Q)
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        let _write_lock = self.write_lock.lock();

        if let Some(node) = self.snapshot() {
            let new_node = node.remove(k).map(Arc::new);
            *self.root.write().unwrap() = new_node;
        }
    }

    /// Get the value associated with the provided key.
    pub fn get<Q>(&self, k: &Q) -> Option<ValueRef<K, V>>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        if let Some(node) = self.snapshot() {
            node.get(k).map(|node| ValueRef { node })
        } else {
            None
        }
    }

    /// Get an iterator over the tree elements.
    pub fn iter(&self) -> Iter<'_, K, V> {
        // Iter's lifetime is bound to &self.
        Iter::new(self.snapshot())
    }

    /// Create a snapshot of the tree (which essentially is reading the current root pointer).
    fn snapshot(&self) -> Option<Arc<Node<K, V>>> {
        // Clone right away to drop the read lock.
        self.root.read().unwrap().clone()
    }
}

/// Reference to a value in the tree.
pub struct ValueRef<K, V> {
    node: Arc<Node<K, V>>,
}

impl<K, V> Deref for ValueRef<K, V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.node.v
    }
}

/// AVL tree node.
#[derive(Clone, Debug)]
struct Node<K, V> {
    /// Key of the key-value pair.
    k: K,
    /// Value of the key-value pair.
    v: V,

    /// Subtree height, rooted in this node.
    h: usize,

    /// Left subtree.
    l: Option<Arc<Node<K, V>>>,

    /// Right subtree.
    r: Option<Arc<Node<K, V>>>,
}

impl<K, V> Node<K, V>
where
    K: Ord + Clone,
    V: Clone,
{
    fn upsert<F>(&self, k: K, f: F) -> Self
    where
        F: FnOnce(Option<&V>) -> V,
    {
        if k < self.k {
            let l = if let Some(l) = &self.l {
                l.upsert(k, f)
            } else {
                Self::leaf(k, f(None))
            };

            return Self {
                l: Some(l).map(Arc::new),
                ..self.clone()
            }
            .recompute_height()
            .rebalance_insert();
        }

        if k > self.k {
            let r = if let Some(r) = &self.r {
                r.upsert(k, f)
            } else {
                Self::leaf(k, f(None))
            };

            return Self {
                r: Some(r).map(Arc::new),
                ..self.clone()
            }
            .recompute_height()
            .rebalance_insert();
        }

        Self {
            k,
            v: f(Some(&self.v)),
            ..self.clone()
        }
    }

    fn update<Q, F>(&self, k: &Q, f: F) -> Option<Self>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
        F: FnOnce(&V) -> V,
    {
        if k < self.k.borrow() {
            let l = self.l.as_ref().and_then(|l| l.update(k, f)).map(Arc::new);

            return Some(Self { l, ..self.clone() });
        }

        if k > self.k.borrow() {
            let r = self.r.as_ref().and_then(|r| r.update(k, f)).map(Arc::new);

            return Some(Self { r, ..self.clone() });
        }

        Some(Self {
            v: f(&self.v),
            ..self.clone()
        })
    }

    fn remove<Q>(&self, k: &Q) -> Option<Self>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        if k < self.k.borrow() {
            return Some(
                Self {
                    l: self.l.as_ref().and_then(|l| l.remove(k).map(Arc::new)),
                    ..self.clone()
                }
                .recompute_height()
                .rebalance_remove(),
            );
        }

        if k > self.k.borrow() {
            return Some(
                Self {
                    r: self.r.as_ref().and_then(|r| r.remove(k).map(Arc::new)),
                    ..self.clone()
                }
                .recompute_height()
                .rebalance_remove(),
            );
        }

        match (&self.l, &self.r) {
            (None, None) => None,
            (None, Some(r)) => Some(r.clone_node()),
            (Some(l), None) => Some(l.clone_node()),
            (Some(l), Some(r)) => {
                let m = l.max();

                Some(
                    Self {
                        l: l.remove(m.k.borrow()).map(Arc::new),
                        r: Some(r.clone()),
                        ..m
                    }
                    .recompute_height()
                    .rebalance_remove(),
                )
            }
        }
    }

    fn get<Q>(self: &Arc<Self>, k: &Q) -> Option<Arc<Self>>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        if k < self.k.borrow() {
            return self.l.as_ref().and_then(|l| l.get(k));
        }

        if k > self.k.borrow() {
            return self.r.as_ref().and_then(|r| r.get(k));
        }

        Some(Arc::clone(self))
    }

    /// Construct a leaf node.
    fn leaf(k: K, v: V) -> Self {
        Self {
            k,
            v,
            h: 1,
            l: None,
            r: None,
        }
    }

    /// Helper to clone the node behind the Arc.
    fn clone_node(self: &Arc<Self>) -> Self {
        (**self).clone()
    }

    /// Compute the balance of the this subtree.
    fn balance(&self) -> isize {
        height(&self.l) as isize - height(&self.r) as isize
    }

    /// Rebalance the subtree after an insert.
    fn rebalance_insert(self) -> Self {
        let balance = self.balance();

        let l_key = self.l.as_ref().map(|l| &l.k);
        let r_key = self.r.as_ref().map(|r| &r.k);

        if balance > 1 && Some(&self.k) > l_key {
            return self.rotate_right();
        }

        if balance < -1 && Some(&self.k) < r_key {
            return self.rotate_left();
        }

        if balance > 1 && Some(&self.k) > l_key {
            return Self {
                l: self.l.as_ref().map(|l| l.rotate_left()).map(Arc::new),
                ..self
            }
            .recompute_height()
            .rotate_right();
        }

        if balance < -1 && Some(&self.k) < r_key {
            return Self {
                r: self.r.as_ref().map(|r| r.rotate_right()).map(Arc::new),
                ..self
            }
            .recompute_height()
            .rotate_left();
        }

        self
    }

    /// Rebalance the subtree after a remove.
    fn rebalance_remove(self) -> Self {
        let balance = self.balance();

        let l_balance = self.l.as_ref().map(|l| l.balance()).unwrap_or(0);
        let r_balance = self.r.as_ref().map(|r| r.balance()).unwrap_or(0);

        if balance > 1 && l_balance >= 0 {
            return self.rotate_right();
        }

        if balance > 1 && l_balance < 0 {
            return Self {
                l: self.l.as_ref().map(|l| l.rotate_left()).map(Arc::new),
                ..self
            }
            .recompute_height()
            .rotate_right();
        }

        if balance < -1 && r_balance <= 0 {
            return self.rotate_left();
        }

        if balance < -1 && r_balance > 0 {
            return Self {
                r: self.r.as_ref().map(|r| r.rotate_right()).map(Arc::new),
                ..self
            }
            .recompute_height()
            .rotate_left();
        }

        self
    }

    /// Node with the max key in this subtree.
    fn max(&self) -> Self {
        if let Some(r) = &self.r {
            r.max()
        } else {
            self.clone()
        }
    }

    /// Rotate the tree left with the pivot of `self`.
    fn rotate_left(&self) -> Self {
        if let Some(r) = &self.r {
            Self {
                l: Some(
                    Self {
                        r: r.l.clone(),
                        ..self.clone()
                    }
                    .recompute_height(),
                )
                .map(Arc::new),
                r: r.r.clone(),
                ..r.clone_node()
            }
            .recompute_height()
        } else {
            self.clone()
        }
    }

    /// Rotate the tree right with the pivot of `self`.
    fn rotate_right(&self) -> Self {
        if let Some(l) = &self.l {
            Self {
                r: Some(
                    Self {
                        l: l.r.clone(),
                        ..self.clone()
                    }
                    .recompute_height(),
                )
                .map(Arc::new),
                l: l.l.clone(),
                ..l.clone_node()
            }
            .recompute_height()
        } else {
            self.clone()
        }
    }

    /// Return the current node with its height recomputed.
    fn recompute_height(self) -> Self {
        Self {
            h: 1 + cmp::max(height(&self.l), height(&self.r)),
            ..self
        }
    }
}

/// Helper to compute a subtree height.
fn height<K, V>(node: &Option<Arc<Node<K, V>>>) -> usize {
    node.as_ref().map(|n| n.h).unwrap_or(0)
}

/// Iterator over the AVL tree key-value pairs.
pub struct Iter<'a, K: 'a, V: 'a> {
    _snapshot: Option<Arc<Node<K, V>>>,
    next_stack: Vec<&'a Node<K, V>>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V>
where
    K: Ord,
{
    type Item = (&'a K, &'a V);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(node) = self.next_stack.pop() {
            if node.r.is_some() {
                self.traverse_left(node.r.as_deref());
            }

            return Some((&node.k, &node.v));
        }

        None
    }
}

impl<'a, K, V> Iter<'a, K, V>
where
    K: Ord,
{
    fn new(snapshot: Option<Arc<Node<K, V>>>) -> Self {
        let mut iter = Self {
            _snapshot: snapshot.clone(),
            next_stack: Vec::new(),
        };

        iter.traverse_left(unsafe {
            snapshot
                .as_deref()
                // Reassign the lifetime to 'a. We know this is safe since we bind the lifetime
                // of `Iter` to `Avl` in [`Avl::iter`] as well as keeping the snapshot of an iterated
                // version of the tree alive as long as the `Iter` instance.
                .map(|s| std::mem::transmute::<&'_ Node<K, V>, &'a Node<K, V>>(s))
        });
        iter
    }

    /// Dive into the left-most node of the given subtree.
    fn traverse_left(&mut self, mut node: Option<&'a Node<K, V>>) {
        while let Some(current) = node {
            self.next_stack.push(current);
            node = current.l.as_deref();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{Avl, Node};

    #[test]
    fn inserted_data_is_gettable() {
        let avl = Avl::new();

        avl.insert("Hello, world!".to_owned(), 20);

        assert_eq!(avl.get("Hello, world!").as_deref(), Some(&20));
    }

    #[test]
    fn inserted_bulk_of_data_all_are_accessible() {
        let pairs = [("a", 1), ("b", 2), ("c", 3), ("d", 4)];
        let avl = Avl::new();

        pairs.iter().for_each(|&(k, v)| avl.insert(k.to_owned(), v));
        pairs
            .iter()
            .for_each(|&(k, v)| assert_eq!(avl.get(k).as_deref(), Some(&v)));
    }

    #[test]
    fn inserted_bulk_of_data_tree_is_balanced() {
        let pairs = [
            ("a", 1),
            ("b", 2),
            ("c", 3),
            ("d", 4),
            ("e", 5),
            ("f", 6),
            ("g", 7),
            ("h", 8),
            ("i", 9),
            ("j", 10),
            ("k", 11),
            ("l", 12),
            ("m", 13),
        ];
        let avl = Avl::new();

        pairs.iter().for_each(|&(k, v)| avl.insert(k.to_owned(), v));

        let snapshot = avl.snapshot().unwrap();
        assert_eq!(
            snapshot.l.as_ref().map(|l| l.h),
            snapshot.r.as_ref().map(|r| r.h)
        );
    }

    #[test]
    fn inserted_bulk_of_data_deleted_some_remaining_are_accessible_and_balanced() {
        let pairs = [
            ("a", 1),
            ("b", 2),
            ("c", 3),
            ("d", 4),
            ("e", 5),
            ("f", 6),
            ("g", 7),
            ("h", 8),
            ("i", 9),
        ];
        let avl = Avl::new();

        pairs.iter().for_each(|&(k, v)| avl.insert(k.to_owned(), v));

        avl.remove("b");
        avl.remove("h");
        avl.remove("i");

        let snapshot = avl.snapshot().unwrap();
        assert_eq!(
            snapshot.l.as_ref().map(|l| l.h),
            snapshot.r.as_ref().map(|r| r.h)
        );

        pairs
            .iter()
            .filter(|(k, _v)| k != &"b" && k != &"h" && k != &"i")
            .for_each(|&(k, v)| assert_eq!(avl.get(k).as_deref(), Some(&v)));

        assert_eq!(avl.get("b").as_deref(), None);
        assert_eq!(avl.get("h").as_deref(), None);
        assert_eq!(avl.get("i").as_deref(), None);
    }

    #[test]
    fn traverse_in_sorted_order() {
        let pairs_unordered = [("b", 2), ("d", 4), ("a", 1), ("c", 3)];
        let expected_order = [("a", 1), ("b", 2), ("c", 3), ("d", 4)];
        let avl = Avl::new();

        pairs_unordered
            .iter()
            .for_each(|&(k, v)| avl.insert(k.to_owned(), v));

        let actual_order = avl
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect::<Vec<_>>();

        assert_eq!(actual_order.as_slice(), expected_order);
    }

    #[test]
    fn iter_walks_the_tree() {
        let pairs = [("a", 1), ("b", 2), ("c", 3), ("d", 4)];
        let avl = Avl::new();

        pairs.iter().for_each(|&(k, v)| avl.insert(k.to_owned(), v));

        assert_eq!(avl.iter().count(), 4);

        let mut iter = avl.iter();

        assert_eq!(iter.next(), Some((&"a".to_owned(), &1)));
        assert_eq!(iter.next(), Some((&"b".to_owned(), &2)));
        assert_eq!(iter.next(), Some((&"c".to_owned(), &3)));
        assert_eq!(iter.next(), Some((&"d".to_owned(), &4)));
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn node_rebalance_insert() {
        let node = Node {
            k: 1,
            v: 1,
            h: 3,
            l: None,
            r: Some(Node {
                k: 2,
                v: 2,
                h: 2,
                l: None,
                r: Some(Node {
                    k: 3,
                    v: 3,
                    h: 1,
                    l: None,
                    r: None,
                })
                .map(Arc::new),
            })
            .map(Arc::new),
        };

        let balanced = node.rebalance_insert();

        assert_eq!(balanced.h, 2);
        assert_eq!(balanced.k, 2);
        assert_eq!(balanced.l.as_ref().unwrap().k, 1);
        assert_eq!(balanced.r.as_ref().unwrap().k, 3);
    }
}
