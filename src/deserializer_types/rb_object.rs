use std::ops::{Deref, DerefMut};

use serde::de::{Deserializer, SeqAccess, Visitor};
use std::marker::PhantomData;

/// Type for deserializing Ruby Object (`o`) values
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RbObject<T, N = ()> {
    pub class: N,
    pub fields: T,
}

impl<T, N> Deref for RbObject<T, N> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.fields
    }
}

impl<T, N> DerefMut for RbObject<T, N> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.fields
    }
}

impl<'de, T, N> serde::Deserialize<'de> for RbObject<T, N>
where
    T: serde::Deserialize<'de>,
    N: serde::Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RbObjectVisitor<T, N>(PhantomData<(T, N)>);

        impl<'de, T, N> Visitor<'de> for RbObjectVisitor<T, N>
        where
            T: serde::Deserialize<'de>,
            N: serde::Deserialize<'de>,
        {
            type Value = RbObject<T, N>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a Ruby Object (2-element sequence: class name, ivars map)")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let class = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let fields = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                Ok(RbObject { class, fields })
            }
        }

        deserializer.deserialize_tuple(2, RbObjectVisitor(PhantomData))
    }
}

/// Type alias for Ruby marshal struct values
pub type RbStruct<T, N = ()> = RbObject<T, N>;
