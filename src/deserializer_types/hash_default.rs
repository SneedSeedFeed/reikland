use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use serde_core::de::{Deserializer, SeqAccess, Visitor};

use super::ignored::Ignored;

/// Deserializer type for Ruby Hash-with-default values
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RbHashDefault<T, D = Ignored> {
    pub hash: T,
    pub default: D,
}

impl<T, D> Deref for RbHashDefault<T, D> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.hash
    }
}

impl<T, D> DerefMut for RbHashDefault<T, D> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.hash
    }
}

impl<'de, T, D> serde_core::Deserialize<'de> for RbHashDefault<T, D>
where
    T: serde_core::Deserialize<'de>,
    D: serde_core::Deserialize<'de>,
{
    fn deserialize<De: Deserializer<'de>>(deserializer: De) -> Result<Self, De::Error> {
        struct RbHashDefaultVisitor<T, D>(PhantomData<(T, D)>);

        impl<'de, T, D> Visitor<'de> for RbHashDefaultVisitor<T, D>
        where
            T: serde_core::Deserialize<'de>,
            D: serde_core::Deserialize<'de>,
        {
            type Value = RbHashDefault<T, D>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a Ruby Hash with default (2-element sequence: hash, default)")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let hash = seq
                    .next_element()?
                    .ok_or_else(|| serde_core::de::Error::invalid_length(0, &self))?;
                let default = seq
                    .next_element()?
                    .ok_or_else(|| serde_core::de::Error::invalid_length(1, &self))?;
                Ok(RbHashDefault { hash, default })
            }
        }

        deserializer.deserialize_tuple(2, RbHashDefaultVisitor(PhantomData))
    }
}
