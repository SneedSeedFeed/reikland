use std::{
    num::NonZeroUsize,
    ops::{Index, IndexMut},
};

// first byte is always the version number so we can use NonZeroUsize to allow for Option<T> to niche optimise
/// Tracks the byte every marshal value starts at, 1 indexed because marshal is <insert vomit emoji>
pub(crate) type ValueTracker = OneIndexedVec<NonZeroUsize>;

pub(crate) struct OneIndexedVec<T> {
    inner: Vec<T>,
}

impl<T> OneIndexedVec<T> {
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            inner: Vec::with_capacity(cap),
        }
    }

    pub fn push(&mut self, item: T) {
        self.inner.push(item)
    }

    pub fn get(&mut self, idx: NonZeroUsize) -> Option<&T> {
        self.inner.get(idx.get() - 1)
    }

    pub fn get_copy(&mut self, idx: NonZeroUsize) -> Option<T>
    where
        T: Copy,
    {
        self.inner.get(idx.get() - 1).copied()
    }
}

impl<T> Index<NonZeroUsize> for OneIndexedVec<T> {
    type Output = T;

    fn index(&self, index: NonZeroUsize) -> &Self::Output {
        self.inner.index(index.get() - 1)
    }
}

impl<T> IndexMut<NonZeroUsize> for OneIndexedVec<T> {
    fn index_mut(&mut self, index: NonZeroUsize) -> &mut Self::Output {
        self.inner.index_mut(index.get() - 1)
    }
}
