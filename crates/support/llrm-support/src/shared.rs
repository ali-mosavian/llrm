//! A value shared until someone changes it.

use std::ops::Deref;
use std::rc::Rc;

/// A value many owners hold, copied only when one of them changes it and others
/// still hold it: LLVM's copy-on-write sets, for the maps a dataflow carries
/// from block to block. Cloning is a count; reading is free; `amend` copies
/// only when there is something to change.
#[derive(Debug, Default)]
pub struct Shared<T>(Rc<T>);

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}

impl<T> Deref for Shared<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: PartialEq> PartialEq for Shared<T> {
    fn eq(
        &self,
        other: &Self,
    ) -> bool {
        Rc::ptr_eq(&self.0, &other.0) || *self.0 == *other.0
    }
}

impl<T> Shared<T> {
    pub fn new(value: T) -> Self {
        Self(Rc::new(value))
    }

    /// Whether both are the one value (and so equal, unread).
    pub fn ptr_eq(
        &self,
        other: &Self,
    ) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl<T: Clone> Shared<T> {
    /// `apply` the change `plan` finds to make by reading the value, and
    /// nothing where it finds none: the value is copied, if others hold it,
    /// only then. Whether it changed.
    pub fn amend<P>(
        &mut self,
        plan: impl FnOnce(&T) -> Option<P>,
        apply: impl FnOnce(&mut T, P),
    ) -> bool {
        let Some(change) = plan(&self.0) else { return false };
        apply(Rc::make_mut(&mut self.0), change);
        true
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::Shared;

    thread_local! {
        static COPIES: Cell<usize> = const { Cell::new(0) };
    }

    #[derive(Debug, PartialEq)]
    struct Counted(Vec<i32>);

    impl Clone for Counted {
        fn clone(&self) -> Self {
            COPIES.with(|copies| copies.set(copies.get() + 1));
            Self(self.0.clone())
        }
    }

    #[test]
    fn a_change_copies_a_shared_value_once_and_no_change_copies_nothing() {
        COPIES.with(|copies| copies.set(0));
        let mut one = Shared::new(Counted(vec![1, 2, 3]));
        let two = one.clone();
        assert!(!one.amend(|held| held.0.contains(&9).then_some(()), |held, ()| held.0.clear()));
        assert_eq!(COPIES.with(Cell::get), 0, "nothing to change, nothing copied");
        assert!(one.ptr_eq(&two));
        assert!(one.amend(|held| Some(held.0.len()), |held, len| held.0.push(len as i32)));
        assert_eq!(COPIES.with(Cell::get), 1, "changed while shared: copied once");
        assert_eq!((one.0.clone(), two.0.clone()), (vec![1, 2, 3, 3], vec![1, 2, 3]));
        COPIES.with(|copies| copies.set(0));
        assert!(one.amend(|_| Some(7), |held, n| held.0.push(n)));
        assert_eq!(COPIES.with(Cell::get), 0, "changed while alone: in place");
    }

    #[test]
    fn equal_when_the_one_value_or_equal_values() {
        let one = Shared::new(Counted(vec![1]));
        assert!(one == one.clone());
        assert!(one == Shared::new(Counted(vec![1])));
        assert!(one != Shared::new(Counted(vec![2])));
    }
}
