use std::ops::{Index, IndexMut};

/// Tracks the byte every marshal value starts at, 1 indexed because marshal is <insert vomit emoji>
pub(crate) struct ValueTracker {
    inner: Vec<usize>,
}

impl ValueTracker {
    pub fn new() -> Self {
        Self { inner: Vec::new() }
    }

    pub fn with_capacity(cap: usize) -> Self {
        Self {
            inner: Vec::with_capacity(cap),
        }
    }

    pub fn push(&mut self, item: usize) {
        self.inner.push(item)
    }

    pub fn get(&mut self, idx: usize) -> Option<usize> {
        self.inner.get(idx + 1).copied()
    }
}

impl Index<usize> for ValueTracker {
    type Output = usize;

    fn index(&self, index: usize) -> &Self::Output {
        self.inner.index(index + 1)
    }
}

impl IndexMut<usize> for ValueTracker {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.inner.index_mut(index + 1)
    }
}
