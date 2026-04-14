use std::ops::Index;

use crate::types::value::MarshalValue;

/// Index into an [`ObjectTable`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectIdx(usize);

impl ObjectIdx {
    pub fn inner(&self) -> usize {
        self.0
    }

    pub fn new(idx: usize) -> Self {
        Self(idx)
    }
}

/// INdex of a marshal object, as extracted from the marshal data
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub struct ObjectRefIdx(usize);

impl ObjectRefIdx {
    pub fn inner(&self) -> usize {
        self.0
    }

    pub fn new(idx: usize) -> Self {
        Self(idx)
    }
}

/// Storage of all objects and object references for a marshal value
#[derive(Debug, Clone, Default)]
pub struct ObjectTable<'a> {
    objects: Vec<MarshalValue<'a>>,
    object_refs: Vec<ObjectIdx>,
}

impl<'a> ObjectTable<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    /// Push an object into this table, returning its [`ObjectIdx`]
    pub fn push_object(&mut self, value: MarshalValue<'a>) -> ObjectIdx {
        let idx = ObjectIdx::new(self.objects.len());
        self.objects.push(value);
        idx
    }

    /// Push an object reference into this table
    pub fn push_object_ref(&mut self, idx: ObjectIdx) {
        self.object_refs.push(idx);
    }

    /// How many object references are currently registered.
    pub fn object_ref_count(&self) -> usize {
        self.object_refs.len()
    }

    /// Replace the most recently pushed object reference with a new target.
    ///
    /// # Panics
    /// Panics if the object reference table is empty.
    pub fn replace_last_object_ref(&mut self, idx: ObjectIdx) {
        *self
            .object_refs
            .last_mut()
            .expect("no object ref to replace") = idx;
    }

    /// Resolve a marshal '@' reference index into an [`ObjectIdx`]
    pub fn get_by_ref(&self, ref_idx: usize) -> Option<ObjectIdx> {
        self.object_refs.get(ref_idx).copied()
    }

    /// Fully resolve an [`ObjectRefIdx`] into its [`MarshalValue`].
    pub fn resolve_ref(&self, ref_idx: ObjectRefIdx) -> Option<&MarshalValue<'a>> {
        let obj_idx = self.object_refs.get(ref_idx.inner())?;
        self.objects.get(obj_idx.inner())
    }
}

impl<'a> Index<usize> for ObjectTable<'a> {
    type Output = MarshalValue<'a>;

    fn index(&self, index: usize) -> &Self::Output {
        self.objects.index(index)
    }
}

impl<'a> Index<ObjectIdx> for ObjectTable<'a> {
    type Output = MarshalValue<'a>;

    fn index(&self, index: ObjectIdx) -> &Self::Output {
        self.objects.index(index.inner())
    }
}

impl<'a> Index<ObjectRefIdx> for ObjectTable<'a> {
    type Output = MarshalValue<'a>;

    fn index(&self, index: ObjectRefIdx) -> &Self::Output {
        self.index(*self.object_refs.index(index.inner()))
    }
}
