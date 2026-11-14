//! [`Shared`]: setup data a cloned [`crate::sim::Sim`] shares with its
//! original (issue #168, `analysis/plan/LAN_MULTIPLAYER.md` §6: a peer
//! clones its confirmed sim every confirmed tick to predict its own soldier,
//! which is cheap because static map data sits behind an `Arc`).
//!
//! Rules:
//! - Reading derefs to the value, so a `Shared<Vec<T>>` reads as the `Vec`
//!   it holds (indexing, `iter`, `len`, `for x in &shared`).
//! - Writing (`DerefMut`, `&mut` iteration) is copy-on-write
//!   ([`std::sync::Arc::make_mut`]): a value shared with a clone is copied
//!   first, so a clone never sees its original's later edits or the reverse.
//!   Only setup and tests write; a step never does, so a clone stays shared.
//! - It is never part of [`crate::sim::Sim::hash`]: what it holds is the same
//!   in both copies, and equality compares values, not pointers.

use std::ops::{Deref, DerefMut};
use std::sync::Arc;

/// A value behind an `Arc`, cloned by reference and written copy-on-write.
pub struct Shared<T>(Arc<T>);

impl<T> Shared<T> {
    pub fn new(value: T) -> Self {
        Shared(Arc::new(value))
    }

    /// Do `a` and `b` hold the same allocation (a clone that nothing has
    /// written since)?
    pub fn ptr_eq(a: &Self, b: &Self) -> bool {
        Arc::ptr_eq(&a.0, &b.0)
    }
}

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(Arc::clone(&self.0))
    }
}

impl<T> Deref for Shared<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Clone> DerefMut for Shared<T> {
    fn deref_mut(&mut self) -> &mut T {
        Arc::make_mut(&mut self.0)
    }
}

impl<T: Default> Default for Shared<T> {
    fn default() -> Self {
        Shared::new(T::default())
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for Shared<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (**self).fmt(f)
    }
}

impl<T: PartialEq> PartialEq for Shared<T> {
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl<T> From<T> for Shared<T> {
    fn from(value: T) -> Self {
        Shared::new(value)
    }
}

impl<A> FromIterator<A> for Shared<Vec<A>> {
    fn from_iter<I: IntoIterator<Item = A>>(iter: I) -> Self {
        Shared::new(iter.into_iter().collect())
    }
}

impl<'a, T> IntoIterator for &'a Shared<T>
where
    &'a T: IntoIterator,
{
    type Item = <&'a T as IntoIterator>::Item;
    type IntoIter = <&'a T as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        (&**self).into_iter()
    }
}

impl<'a, T: Clone> IntoIterator for &'a mut Shared<T>
where
    &'a mut T: IntoIterator,
{
    type Item = <&'a mut T as IntoIterator>::Item;
    type IntoIter = <&'a mut T as IntoIterator>::IntoIter;
    fn into_iter(self) -> Self::IntoIter {
        (&mut **self).into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::Shared;

    #[test]
    fn shared_clones_by_reference_and_writes_copy_on_write() {
        let a: Shared<Vec<u32>> = (0..4).collect();
        let mut b = a.clone();
        assert!(Shared::ptr_eq(&a, &b));
        assert_eq!(b[2], 2);
        assert_eq!((&b).into_iter().sum::<u32>(), 6);
        b.push(9);
        assert!(!Shared::ptr_eq(&a, &b));
        assert_eq!((a.len(), b.len()), (4, 5));
        for x in &mut b {
            *x += 1;
        }
        assert_eq!(a[0], 0);
        assert_eq!(b[0], 1);
    }
}
