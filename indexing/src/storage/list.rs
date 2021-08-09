use std::sync::Arc;

/// Immutable singly-linked list.
#[derive(Clone)]
pub(crate) enum List<T> {
    Null,
    Cons(T, Arc<List<T>>),
}

impl<T> List<T>
where
    T: Clone,
{
    pub fn iter(&self) -> Iter<'_, T> {
        Iter { list: self }
    }
}

pub(crate) struct Iter<'a, T: 'a> {
    list: &'a List<T>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        match &*self.list {
            List::Null => None,
            List::Cons(head, tail) => {
                self.list = tail;
                Some(head)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::List;

    #[test]
    fn constructed_list_is_traversable() {
        let lst = List::Cons(
            1,
            Arc::new(List::Cons(2, Arc::new(List::Cons(3, Arc::new(List::Null))))),
        );

        let mut iter = lst.iter();

        assert_eq!(iter.next(), Some(&1));
        assert_eq!(iter.next(), Some(&2));
        assert_eq!(iter.next(), Some(&3));
        assert_eq!(iter.next(), None);
    }
}
