use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use serde::de::{Deserializer, IntoDeserializer, SeqAccess, Visitor};

/// A newtype wrapper that transparently handles sequence-wrapped values such as instance variables by taking the first member of teh sequence then draining the remainder
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Transparent<T>(pub T);

impl<T> Deref for Transparent<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T> DerefMut for Transparent<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

impl<'de, T> serde::Deserialize<'de> for Transparent<T>
where
    T: serde::Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct TransparentVisitor<T>(PhantomData<T>);

        impl<'de, T> Visitor<'de> for TransparentVisitor<T>
        where
            T: serde::Deserialize<'de>,
        {
            type Value = Transparent<T>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a value, or a sequence wrapping a value")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let inner: T = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                // drain remaining elements
                while seq.next_element::<serde::de::IgnoredAny>()?.is_some() {}
                Ok(Transparent(inner))
            }

            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_i8<E: serde::de::Error>(self, v: i8) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_i16<E: serde::de::Error>(self, v: i16) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_i32<E: serde::de::Error>(self, v: i32) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_i128<E: serde::de::Error>(self, v: i128) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_u8<E: serde::de::Error>(self, v: u8) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_u16<E: serde::de::Error>(self, v: u16) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_u32<E: serde::de::Error>(self, v: u32) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_u128<E: serde::de::Error>(self, v: u128) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_f32<E: serde::de::Error>(self, v: f32) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_char<E: serde::de::Error>(self, v: char) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_borrowed_str<E: serde::de::Error>(
                self,
                v: &'de str,
            ) -> Result<Self::Value, E> {
                T::deserialize(serde::de::value::BorrowedStrDeserializer::new(v)).map(Transparent)
            }

            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_borrowed_bytes<E: serde::de::Error>(
                self,
                v: &'de [u8],
            ) -> Result<Self::Value, E> {
                T::deserialize(serde::de::value::BorrowedBytesDeserializer::new(v)).map(Transparent)
            }

            fn visit_byte_buf<E: serde::de::Error>(self, v: Vec<u8>) -> Result<Self::Value, E> {
                T::deserialize(v.into_deserializer()).map(Transparent)
            }

            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                T::deserialize(().into_deserializer()).map(Transparent)
            }

            fn visit_none<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                T::deserialize(().into_deserializer()).map(Transparent)
            }

            fn visit_some<D: Deserializer<'de>>(
                self,
                deserializer: D,
            ) -> Result<Self::Value, D::Error> {
                T::deserialize(deserializer).map(Transparent)
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<Self::Value, A::Error> {
                T::deserialize(serde::de::value::MapAccessDeserializer::new(map)).map(Transparent)
            }
        }

        deserializer.deserialize_any(TransparentVisitor(PhantomData))
    }
}
