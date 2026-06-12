use std::{
    num::NonZeroUsize,
    ops::{Index, IndexMut},
};

/// Tracks the byte every marshal value starts at, 1 indexed because marshal is <insert vomit emoji>
pub(crate) type ValueTracker = OneIndexedVec<usize>;

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
