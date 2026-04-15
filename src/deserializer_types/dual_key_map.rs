use std::{
    collections::HashMap, hash::Hash, iter::FilterMap, marker::PhantomData, mem::MaybeUninit,
};

use bit_vec::BitVec;
use serde::{
    Deserialize, Serialize,
    de::{IgnoredAny, MapAccess, Visitor},
};

use crate::deserializer_types::MixedKeyRef;

/// Deserializes a map/hash in ruby that has a [`MixedKey`][super::mixed_key::MixedKey] key, discarding the [MixedKey::Int][super::mixed_key::MixedKey::Int] keys.
/// Does not verify that discarded values aren't unique.
#[derive(Debug, Clone, Serialize)]
#[repr(transparent)]
pub struct DualKeyMap<K, V>(pub HashMap<K, V>);

impl<'de, K, V> serde::Deserialize<'de> for DualKeyMap<K, V>
where
    K: From<&'de str> + Eq + Hash,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct DualKeyMapVisitor<K, V>(PhantomData<fn() -> (K, V)>);

        impl<'de, K, V> Visitor<'de> for DualKeyMapVisitor<K, V>
        where
            K: From<&'de str> + Eq + Hash,
            V: Deserialize<'de>,
        {
            type Value = DualKeyMap<K, V>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a map with mixed string/integer keys")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut hash: HashMap<K, V> = map
                    .size_hint()
                    .map(HashMap::with_capacity)
                    .unwrap_or_default();

                while let Some(key) = map.next_key::<MixedKeyRef<'de>>()? {
                    match key {
                        MixedKeyRef::Int(_) => {
                            map.next_value::<serde::de::IgnoredAny>()?;
                        }
                        MixedKeyRef::Str(s) => {
                            let key = K::from(s);
                            hash.insert(key, map.next_value()?);
                        }
                    }
                }

                Ok(DualKeyMap(hash))
            }
        }

        deserializer.deserialize_map(DualKeyMapVisitor(PhantomData))
    }
}

/// Deserializes a map/hash in ruby that has a [`MixedKey`][super::mixed_key::MixedKey] key, discarding the [MixedKey::Str][super::mixed_key::MixedKey::Str] keys.
/// Does not verify that discarded values aren't unique.
#[derive(Debug, Clone, Serialize)]
#[repr(transparent)]
pub struct DualKeyMapInt<K, V>(pub HashMap<K, V>);

impl<'de, K, V> serde::Deserialize<'de> for DualKeyMapInt<K, V>
where
    K: From<i32> + Eq + Hash,
    V: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct DualKeyMapIntVisitor<K, V>(PhantomData<fn() -> (K, V)>);

        impl<'de, K, V> Visitor<'de> for DualKeyMapIntVisitor<K, V>
        where
            K: From<i32> + Eq + Hash,
            V: Deserialize<'de>,
        {
            type Value = DualKeyMapInt<K, V>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a map with mixed string/integer keys")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut hash: HashMap<K, V> = map
                    .size_hint()
                    .map(HashMap::with_capacity)
                    .unwrap_or_default();

                while let Some(key) = map.next_key::<MixedKeyRef<'de>>()? {
                    match key {
                        MixedKeyRef::Int(i) => {
                            let key = K::from(i);
                            hash.insert(key, map.next_value()?);
                        }
                        MixedKeyRef::Str(_) => {
                            map.next_value::<IgnoredAny>()?;
                        }
                    }
                }

                Ok(DualKeyMapInt(hash))
            }
        }

        deserializer.deserialize_map(DualKeyMapIntVisitor(PhantomData))
    }
}

// this unsafe bullshit was for fun not practicality, cant wait for it to bite me in the ass
struct SparseVecBuilder<T> {
    members: Vec<MaybeUninit<T>>,
    initialised: BitVec<u32>,
}

impl<T> Default for SparseVecBuilder<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> SparseVecBuilder<T> {
    fn new() -> SparseVecBuilder<T> {
        SparseVecBuilder {
            members: Vec::new(),
            initialised: BitVec::new(),
        }
    }

    fn with_capacity(capacity: usize) -> SparseVecBuilder<T> {
        SparseVecBuilder {
            members: Vec::with_capacity(capacity),
            initialised: BitVec::with_capacity(capacity),
        }
    }

    fn insert(&mut self, index: usize, value: T) {
        // todo: growth strategy?
        if index >= self.members.len() {
            let diff = index + 1 - self.members.len();
            self.members.resize_with(index + 1, MaybeUninit::uninit);
            self.initialised.grow(diff, false);
        }

        if self.initialised.get(index).unwrap() {
            // Safety: We have confirmed index is initialised
            unsafe { self.members[index].assume_init_drop() }
        }

        self.members[index] = MaybeUninit::new(value);
        self.initialised.set(index, true);
    }

    fn build(mut self) -> Option<Vec<T>> {
        // if there are any holes we return None and let Drop clean up
        if !self.initialised.all() {
            return None;
        }

        assert_eq!(
            self.initialised.len(),
            self.members.len(),
            "length of vec and bitvec should be identical in sparse builder"
        );

        // we must clear initialised so the Drop impl won't try to drop the members we're taking ownership of
        std::mem::take(&mut self.initialised);
        let mut vec = std::mem::take(&mut self.members);

        let len = vec.len();
        let capacity = vec.capacity();
        let ptr = vec.as_mut_ptr() as *mut T;

        // prevent a double free (thank you miri)
        std::mem::forget(vec);

        // Safety: all members of vec have been confirmed initialised. MaybeUninit<T> and T have identical layouts
        Some(unsafe { Vec::from_raw_parts(ptr, len, capacity) })
    }
}

impl<T> Drop for SparseVecBuilder<T> {
    fn drop(&mut self) {
        for (i, item) in self.members.iter_mut().enumerate() {
            if self.initialised.get(i).unwrap() {
                // Safety: we have confirmed the item is initialised before dropping
                unsafe {
                    item.assume_init_drop();
                }
            }
        }

        // Safety: We have ensured all the init members are dropped already, and are not increasing the len into any potentially uninit memory
        unsafe {
            self.members.set_len(0);
        }
    }
}

/// Deserializes a map/hash in ruby that has a [`MixedKey`][super::mixed_key::MixedKey] key, discarding the [`MixedKey::Str`][super::mixed_key::MixedKey::Str] keys.
/// Does not verify that discarded values aren't unique. DOES maintain the declared index from the format but does not allow holes. For a version that does not maintain indexes but may deserialize faster see [`DualKeyVec`].
/// The OFFSET constant is subtracted from each index, primarily for cases where the deserialized data is 1-indexed to avoid leaving holes at 0.
#[derive(Debug, Clone, Serialize)]
#[repr(transparent)]
pub struct DualKeyVecSparse<T, const OFFSET: usize = 0>(pub Vec<T>);

impl<'de, T, const OFFSET: usize> serde::Deserialize<'de> for DualKeyVecSparse<T, OFFSET>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct DualKeyVecVisitor<T, const OFFSET: usize>(PhantomData<fn() -> T>);

        impl<'de, T, const OFFSET: usize> Visitor<'de> for DualKeyVecVisitor<T, OFFSET>
        where
            T: Deserialize<'de>,
        {
            type Value = Vec<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str(
                    "a map with mixed string/integer keys and contiguous integer indexes",
                )
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut vec = map
                    .size_hint()
                    .map(SparseVecBuilder::<T>::with_capacity)
                    .unwrap_or_default();

                while let Some(key) = map.next_key::<MixedKeyRef<'de>>()? {
                    let MixedKeyRef::Int(key) = key else {
                        map.next_value::<IgnoredAny>()?;
                        continue;
                    };
                    let index = usize::try_from(key).map_err(|_| {
                        serde::de::Error::custom(format_args!(
                            "key '{key}' should be a positive integer"
                        ))
                    })?;
                    if OFFSET > index {
                        return Err(serde::de::Error::custom(
                            "overflow when subtracting OFFSET from index",
                        ));
                    }
                    let index = index - OFFSET;
                    vec.insert(index, map.next_value::<T>()?);
                }

                vec.build().ok_or_else(|| {
                    serde::de::Error::custom(
                        "DualKeyVecSparse found holes in indexes of source data",
                    )
                })
            }
        }

        deserializer
            .deserialize_map(DualKeyVecVisitor::<T, OFFSET>(PhantomData))
            .map(DualKeyVecSparse)
    }
}

/// Deserializes a map/hash in ruby that has a [`MixedKey`][super::mixed_key::MixedKey] key, discarding the [`MixedKey::Str`][super::mixed_key::MixedKey::Str] keys.
/// Does not verify that discarded values aren't unique. Does not maintain the declared index from the format as it just inserts each value in the order they are encountered.
#[derive(Debug, Clone, Serialize)]
#[repr(transparent)]
pub struct DualKeyVec<T>(pub Vec<T>);

impl<'de, T> serde::Deserialize<'de> for DualKeyVec<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct DualKeyVecVisitor<T>(PhantomData<fn() -> T>);

        impl<'de, T> Visitor<'de> for DualKeyVecVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = DualKeyVec<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a map with mixed string/integer keys")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut vec = map.size_hint().map(Vec::with_capacity).unwrap_or_default();

                while let Some(key) = map.next_key::<MixedKeyRef<'de>>()? {
                    let MixedKeyRef::Int(_) = key else {
                        map.next_value::<IgnoredAny>()?;
                        continue;
                    };

                    vec.push(map.next_value::<T>()?);
                }

                Ok(DualKeyVec(vec))
            }
        }

        deserializer.deserialize_map(DualKeyVecVisitor(PhantomData))
    }
}

/// Deserializes a map/hash in ruby that has a [`MixedKey`][super::mixed_key::MixedKey] key, discarding the [`MixedKey::Str`][super::mixed_key::MixedKey::Str] keys.
/// Does not verify that discarded values aren't unique. DOES maintain the declared index from the format and permits holes.
#[derive(Debug, Clone, Serialize)]
#[repr(transparent)]
pub struct DualKeyVecSparseHoley<T>(pub Vec<Option<T>>);

impl<'de, T> serde::Deserialize<'de> for DualKeyVecSparseHoley<T>
where
    T: Deserialize<'de>,
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct DualKeyVecSparseHoleyVisitor<T>(PhantomData<fn() -> T>);

        impl<'de, T> Visitor<'de> for DualKeyVecSparseHoleyVisitor<T>
        where
            T: Deserialize<'de>,
        {
            type Value = DualKeyVecSparseHoley<T>;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("a map with mixed string/integer keys")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: serde::de::MapAccess<'de>,
            {
                let mut vec = map
                    .size_hint()
                    .map(Vec::<Option<T>>::with_capacity)
                    .unwrap_or_default();

                while let Some(key) = map.next_key::<MixedKeyRef<'de>>()? {
                    let MixedKeyRef::Int(key) = key else {
                        map.next_value::<IgnoredAny>()?;
                        continue;
                    };
                    let index = usize::try_from(key).map_err(|_| {
                        serde::de::Error::custom(format_args!(
                            "key '{key}' should be a positive integer"
                        ))
                    })?;
                    let value = map.next_value::<T>()?;
                    if index >= vec.len() {
                        vec.resize_with(index + 1, || None);
                    }
                    vec[index] = Some(value);
                }

                Ok(DualKeyVecSparseHoley(vec))
            }
        }

        deserializer.deserialize_map(DualKeyVecSparseHoleyVisitor(PhantomData))
    }
}

pub type SparseHoleyIter<'a, T> =
    FilterMap<std::slice::Iter<'a, Option<T>>, fn(&Option<T>) -> Option<&T>>;
impl<T> DualKeyVecSparseHoley<T> {
    pub fn iter_filled(&self) -> SparseHoleyIter<'_, T> {
        self.0.iter().filter_map(|s| s.as_ref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_vec_builder_sequential_insert() {
        let mut builder = SparseVecBuilder::new();
        builder.insert(0, String::from("a"));
        builder.insert(1, String::from("b"));
        builder.insert(2, String::from("c"));
        let result = builder.build().unwrap();
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn sparse_vec_builder_reverse_insert() {
        let mut builder = SparseVecBuilder::new();
        builder.insert(2, String::from("c"));
        builder.insert(1, String::from("b"));
        builder.insert(0, String::from("a"));
        let result = builder.build().unwrap();
        assert_eq!(result, vec!["a", "b", "c"]);
    }

    #[test]
    fn sparse_vec_builder_with_hole_returns_none() {
        let mut builder = SparseVecBuilder::new();
        builder.insert(0, String::from("a"));
        builder.insert(2, String::from("c"));
        // index 1 is missing
        assert!(builder.build().is_none());
    }

    #[test]
    fn sparse_vec_builder_overwrite_drops_old_value() {
        let mut builder = SparseVecBuilder::new();
        builder.insert(0, String::from("first"));
        builder.insert(0, String::from("second"));
        let result = builder.build().unwrap();
        assert_eq!(result, vec!["second"]);
    }

    #[test]
    fn sparse_vec_builder_empty_builds_to_empty() {
        let builder = SparseVecBuilder::<String>::new();
        let result = builder.build().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn sparse_vec_builder_drop_with_holes() {
        // Exercises the Drop impl with uninit gaps — Miri will catch
        // use-of-uninit or double-free here.
        let mut builder = SparseVecBuilder::new();
        builder.insert(0, vec![1, 2, 3]);
        builder.insert(3, vec![4, 5, 6]);
        // indices 1, 2 are holes — drop should only touch 0 and 3
        drop(builder);
    }

    #[test]
    fn sparse_vec_builder_drop_after_failed_build() {
        let mut builder = SparseVecBuilder::new();
        builder.insert(0, Box::new(42));
        builder.insert(5, Box::new(99));
        // build returns None due to holes, then the builder is dropped
        let result = builder.build();
        assert!(result.is_none());
    }

    #[test]
    fn sparse_vec_builder_single_element() {
        let mut builder = SparseVecBuilder::new();
        builder.insert(0, Box::new("hello".to_string()));
        let result = builder.build().unwrap();
        assert_eq!(*result[0], "hello");
    }

    #[test]
    fn sparse_vec_builder_large_gap_then_fill() {
        let mut builder = SparseVecBuilder::new();
        // insert at a high index first, forcing resize with many uninit slots
        builder.insert(100, 100i64);
        for i in 0..100 {
            builder.insert(i, i as i64);
        }
        let result = builder.build().unwrap();
        assert_eq!(result.len(), 101);
        assert_eq!(result[0], 0);
        assert_eq!(result[100], 100);
    }

    #[test]
    fn sparse_vec_builder_with_capacity() {
        let mut builder = SparseVecBuilder::with_capacity(4);
        for i in 0..4 {
            builder.insert(i, format!("item_{i}"));
        }
        let result = builder.build().unwrap();
        assert_eq!(result.len(), 4);
    }

    #[test]
    fn sparse_vec_builder_repeated_overwrites() {
        // Hammer one index to check that each overwrite properly drops
        let mut builder = SparseVecBuilder::new();
        builder.insert(0, String::from("v1"));
        for i in 0..50 {
            builder.insert(0, format!("v{}", i + 2));
        }
        let result = builder.build().unwrap();
        assert_eq!(result, vec!["v51"]);
    }
}
