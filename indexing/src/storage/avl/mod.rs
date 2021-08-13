mod mvcc;
mod node;

use std::{borrow::Borrow, ops::Deref, sync::Arc};

pub use mvcc::MvccAvl;

use node::Node;

/// AVL tree implementation.
///
/// This is a self-balancing tree which guarantees the difference in branches height to be no more than one.
/// Thus, the operations on the tree all have `O(log(N))` complexity.
///
/// It stores key-value pairs, with the condition that key implements `Ord` and both key and value are
/// cloneable.
///
/// The implementation is immutable, every modifying operation returns a new tree. Although, parts of
/// the tree that were not touched my the modification are reused.
#[derive(Clone)]
pub struct Avl<K, V> {
    root: Option<Arc<Node<K, V>>>,
}

impl<K, V> Avl<K, V>
where
    K: Ord + Clone,
    V: Clone,
{
    pub fn new() -> Self {
        Self { root: None }
    }

    /// Insert a new key-value pair in the tree.
    ///
    /// If the given key already exists in the tree, its associated value is updated with the newly supplied one.
    pub fn insert(&self, k: K, v: V) -> Self {
        let new_root = if let Some(node) = &self.root {
            Arc::new(node.upsert(k, |_| v))
        } else {
            Arc::new(Node::leaf(k, v))
        };

        Self {
            root: Some(new_root),
        }
    }

    /// Updates or inserts a new key-value pair in the tree.
    ///
    /// If the given key already exists in the tree, its current value is passed to the provided function,
    /// and the returned value will be the new associated with this key value. If the given key does not yet
    /// exist in the tree, a new node will be inserted and the provided function will be called with `None`
    /// to get an initial value to associate with this key.
    pub fn upsert<F>(&self, k: K, f: F) -> Self
    where
        F: FnOnce(Option<&V>) -> V,
    {
        let new_root = if let Some(node) = &self.root {
            Arc::new(node.upsert(k, f))
        } else {
            Arc::new(Node::leaf(k, f(None)))
        };

        Self {
            root: Some(new_root),
        }
    }

    /// Updates an existing value in the tree.
    ///
    /// If the given key exists in the tree, its current value is passed to the provided function and the
    /// returned value will be the new associated with this key value.
    ///
    /// Otherwise, the function is never called and the unmodified tree is returned.
    pub fn update<Q, F>(&self, k: &Q, f: F) -> Self
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
        F: FnOnce(&V) -> V,
    {
        Self {
            root: self
                .root
                .as_deref()
                .and_then(|node| node.update(k, f).map(Arc::new)),
        }
    }

    /// Remove the key-value pair associated with the given key from the tree.
    pub fn remove<Q>(&self, k: &Q) -> Self
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        Self {
            root: self
                .root
                .as_deref()
                .and_then(|node| node.remove(k).map(Arc::new)),
        }
    }

    /// Get the value associated with the provided key.
    pub fn get<Q>(&self, k: &Q) -> Option<ValueRef<K, V>>
    where
        K: Borrow<Q>,
        Q: Ord + ?Sized,
    {
        self.root
            .as_ref()
            .and_then(|node| node.get(k).map(ValueRef::new))
    }

    /// Get an iterator over the tree elements.
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter::new(&self.root)
    }
}

/// Reference to a value in the tree.
pub struct ValueRef<K, V> {
    node: Arc<Node<K, V>>,
}

impl<K, V> ValueRef<K, V> {
    fn new(node: Arc<Node<K, V>>) -> Self {
        Self { node }
    }
}

impl<K, V> Deref for ValueRef<K, V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.node.v
    }
}

pub struct Iter<'a, K, V> {
    next_stack: Vec<&'a Node<K, V>>,
}

impl<'a, K, V> Iterator for Iter<'a, K, V> {
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

impl<'a, K, V> Iter<'a, K, V> {
    fn new(root: &'a Option<Arc<Node<K, V>>>) -> Self {
        let mut iter = Self {
            next_stack: Vec::new(),
        };

        iter.traverse_left(root.as_deref());
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
    use super::Avl;

    #[test]
    fn inserted_data_is_gettable() {
        let avl = Avl::new();
        let avl = avl.insert("Hello, world!".to_owned(), 20);

        assert_eq!(avl.get("Hello, world!").as_deref(), Some(&20));
    }

    #[test]
    fn inserted_bulk_of_data_all_are_accessible() {
        let pairs = [("a", 1), ("b", 2), ("c", 3), ("d", 4)];
        let avl = pairs
            .iter()
            .fold(Avl::new(), |avl, &(k, v)| avl.insert(k.to_owned(), v));

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
        let avl = pairs
            .iter()
            .fold(Avl::new(), |avl, &(k, v)| avl.insert(k.to_owned(), v));

        let root = avl.root.unwrap();
        assert_eq!(root.l.as_ref().map(|l| l.h), root.r.as_ref().map(|r| r.h));
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
        let avl = pairs
            .iter()
            .fold(Avl::new(), |avl, &(k, v)| avl.insert(k.to_owned(), v));

        let avl = avl.remove("b");
        let avl = avl.remove("h");
        let avl = avl.remove("i");

        let root = avl.root.as_deref().unwrap();
        assert_eq!(root.l.as_ref().map(|l| l.h), root.r.as_ref().map(|r| r.h));

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
        let avl = pairs_unordered
            .iter()
            .fold(Avl::new(), |avl, &(k, v)| avl.insert(k.to_owned(), v));

        let actual_order = avl
            .iter()
            .map(|(k, v)| (k.as_str(), *v))
            .collect::<Vec<_>>();

        assert_eq!(actual_order.as_slice(), expected_order);
    }

    #[test]
    fn iter_walks_the_tree() {
        let pairs = [("a", 1), ("b", 2), ("c", 3), ("d", 4)];
        let avl = pairs
            .iter()
            .fold(Avl::new(), |avl, &(k, v)| avl.insert(k.to_owned(), v));

        assert_eq!(avl.iter().count(), 4);

        let mut iter = avl.iter();

        assert_eq!(iter.next(), Some((&"a".to_owned(), &1)));
        assert_eq!(iter.next(), Some((&"b".to_owned(), &2)));
        assert_eq!(iter.next(), Some((&"c".to_owned(), &3)));
        assert_eq!(iter.next(), Some((&"d".to_owned(), &4)));
        assert_eq!(iter.next(), None);
    }
}
