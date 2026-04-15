use std::ops::{Deref, DerefMut};

use serde_core::de::{Deserializer, MapAccess, SeqAccess, Visitor};
use std::marker::PhantomData;

use crate::types::encoding::RubyEncoding;

use super::ignored::Ignored;

/// Type for deserializing Ruby Instance variable wrappers.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ivar<T, O = Ignored> {
    pub inner: T,
    pub ivars: O,
}

impl<T, O> Deref for Ivar<T, O> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.inner
    }
}

impl<T, O> DerefMut for Ivar<T, O> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<'de, T, O> serde_core::Deserialize<'de> for Ivar<T, O>
where
    T: serde_core::Deserialize<'de>,
    O: serde_core::Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct IvarVisitor<T, O>(PhantomData<(T, O)>);

        impl<'de, T, O> Visitor<'de> for IvarVisitor<T, O>
        where
            T: serde_core::Deserialize<'de>,
            O: serde_core::Deserialize<'de>,
        {
            type Value = Ivar<T, O>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an instance variable wrapper (2-element sequence)")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let inner = seq
                    .next_element()?
                    .ok_or_else(|| serde_core::de::Error::invalid_length(0, &self))?;
                let ivars = seq
                    .next_element()?
                    .ok_or_else(|| serde_core::de::Error::invalid_length(1, &self))?;
                Ok(Ivar { inner, ivars })
            }
        }

        deserializer.deserialize_tuple(2, IvarVisitor(PhantomData))
    }
}

// comment is from claude and verifying the behaviour is todo. I just hope nobody relies on this too much and everyone can live in a magic happy utf-8 world
/// Encoding metadata extracted from Ruby Instance variable wrappers.
///
/// Ruby Marshal strings carry encoding info as ivars in one of two forms:
/// - `E: true` → UTF-8, `E: false` → US-ASCII
/// - `encoding: "name"` → a specific [`RubyEncoding`]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Encoding(pub RubyEncoding);

impl Deref for Encoding {
    type Target = RubyEncoding;
    fn deref(&self) -> &RubyEncoding {
        &self.0
    }
}

impl<'de> serde_core::Deserialize<'de> for Encoding {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct EncodingVisitor;

        impl<'de> Visitor<'de> for EncodingVisitor {
            type Value = Encoding;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an ivar map with encoding info (E or encoding key)")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut encoding: Option<RubyEncoding> = None;

                while let Some(key) = map.next_key::<&str>()? {
                    match key {
                        "E" => {
                            let utf8: bool = map.next_value()?;
                            if encoding.is_none() {
                                encoding = Some(if utf8 {
                                    RubyEncoding::Utf8
                                } else {
                                    RubyEncoding::UsAscii
                                });
                            }
                        }
                        "encoding" => {
                            encoding = Some(map.next_value()?);
                        }
                        _ => {
                            map.next_value::<serde_core::de::IgnoredAny>()?;
                        }
                    }
                }

                encoding
                    .map(Encoding)
                    .ok_or_else(|| serde_core::de::Error::missing_field("E or encoding"))
            }
        }

        deserializer.deserialize_map(EncodingVisitor)
    }
}

/// Type alias for an Instance-wrapped value with its encoding resolved
pub type WithEncoding<T> = Ivar<T, Encoding>;
