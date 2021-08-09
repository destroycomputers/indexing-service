//! This module enables a user to intern strings or any other type that implements `Ord` and `Clone`.

use std::{borrow::Borrow, ops::Deref, ptr, sync::Arc};

use crate::storage::Avl;

/// Interned value pool.
pub struct InternPool<T> {
    values: Avl<T, Arc<T>>,
}

impl<T> InternPool<T>
where
    T: Clone + Ord,
{
    /// Create a new instance of [`InternPool`].
    pub fn new() -> Self {
        Self { values: Avl::new() }
    }

    /// Intern a value.
    ///
    /// The [`InternRef`] returned will point to a unique piece of memory for every distinct
    /// value supplied.
    pub(crate) fn intern<K>(&self, value: &K) -> InternRef<T>
    where
        T: Borrow<K>,
        K: ?Sized + Ord + ToOwned<Owned = T>,
    {
        if let Some(reference) = self.values.get(value).as_deref() {
            InternRef(Arc::clone(reference))
        } else {
            let interned = Arc::new(value.to_owned());
            self.values.insert(value.to_owned(), interned.clone());
            InternRef(interned)
        }
    }
}

/// Reference to the interned value.
///
/// The equality of two interned values is implemented as a simple pointer equality check.
#[derive(Debug, Clone, Eq, PartialOrd, Ord)]
pub struct InternRef<T>(Arc<T>);

impl<T> PartialEq for InternRef<T> {
    fn eq(&self, other: &Self) -> bool {
        ptr::eq(Arc::as_ptr(&self.0), Arc::as_ptr(&other.0))
    }
}

impl<T> Deref for InternRef<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
