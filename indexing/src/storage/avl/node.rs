use std::{borrow::Borrow, cmp, sync::Arc};

/// AVL tree node.
#[derive(Clone, Debug)]
pub(crate) struct Node<K, V> {
    /// Key of the key-value pair.
    pub k: K,
    /// Value of the key-value pair.
    pub v: V,

    /// Subtree height, rooted in this node.
    pub h: usize,

    /// Left subtree.
    pub l: Option<Arc<Node<K, V>>>,

    /// Right subtree.
    pub r: Option<Arc<Node<K, V>>>,
}

impl<K, V> Node<K, V>
where
    K: Ord + Clone,
    V: Clone,
{
    pub fn upsert<F>(&self, k: K, f: F) -> Self
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

    pub fn update<Q, F>(&self, k: &Q, f: F) -> Option<Self>
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

    pub fn remove<Q>(&self, k: &Q) -> Option<Self>
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

    pub fn get<Q>(self: &Arc<Self>, k: &Q) -> Option<Arc<Self>>
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
    pub fn leaf(k: K, v: V) -> Self {
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::Node;

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
