use serde_core::de::{DeserializeSeed, MapAccess, Visitor};

use super::{Deserializer, MarshalDeserializeError, rb_str_to_str};

pub(crate) struct HashMapAccess<'de, 'a> {
    de: &'a mut Deserializer<'de>,
    remaining: usize,
    value_pending: bool,
}

impl<'de, 'a> HashMapAccess<'de, 'a> {
    pub(crate) fn new(de: &'a mut Deserializer<'de>, pairs: usize) -> Self {
        HashMapAccess {
            de,
            remaining: pairs,
            value_pending: false,
        }
    }

    /// Parse past whatever the visitor didn't consume.
    pub(crate) fn finish(&mut self) -> Result<(), MarshalDeserializeError> {
        if std::mem::take(&mut self.value_pending) {
            self.de.skip_value()?;
        }
        let remaining = std::mem::take(&mut self.remaining);
        self.de.skip_hash_pairs(remaining)
    }
}

impl<'de> MapAccess<'de> for HashMapAccess<'de, '_> {
    type Error = MarshalDeserializeError;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, MarshalDeserializeError> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        self.value_pending = true;
        seed.deserialize(&mut *self.de).map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        debug_assert!(
            self.value_pending,
            "next_value_seed called before next_key_seed"
        );
        self.value_pending = false;
        seed.deserialize(&mut *self.de)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining)
    }
}

pub(crate) struct IvarMapAccess<'de, 'a> {
    de: &'a mut Deserializer<'de>,
    remaining: usize,
    value_pending: bool,
}

impl<'de, 'a> IvarMapAccess<'de, 'a> {
    pub(crate) fn new(de: &'a mut Deserializer<'de>, pairs: usize) -> Self {
        IvarMapAccess {
            de,
            remaining: pairs,
            value_pending: false,
        }
    }

    pub(crate) fn finish(&mut self) -> Result<(), MarshalDeserializeError> {
        if std::mem::take(&mut self.value_pending) {
            self.de.skip_value()?;
        }
        for _ in 0..std::mem::take(&mut self.remaining) {
            self.de.parse_symbol()?;
            self.de.skip_value()?;
        }
        Ok(())
    }
}

impl<'de> MapAccess<'de> for IvarMapAccess<'de, '_> {
    type Error = MarshalDeserializeError;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, MarshalDeserializeError> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        self.value_pending = true;
        let s = rb_str_to_str(self.de.parse_symbol()?)?;
        seed.deserialize(serde_core::de::value::BorrowedStrDeserializer::new(s))
            .map(Some)
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(
        &mut self,
        seed: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        debug_assert!(
            self.value_pending,
            "next_value_seed called before next_key_seed"
        );
        self.value_pending = false;
        seed.deserialize(&mut *self.de)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining)
    }
}

pub(crate) struct SymbolNameDeserializer<'de> {
    pub(crate) name: &'de str,
}

impl<'de> serde_core::de::Deserializer<'de> for SymbolNameDeserializer<'de> {
    type Error = MarshalDeserializeError;

    fn deserialize_any<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        visitor.visit_borrowed_str(self.name)
    }

    fn deserialize_str<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.deserialize_any(visitor)
    }

    fn deserialize_string<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.deserialize_any(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        visitor.visit_unit()
    }

    fn deserialize_unit<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        visitor.visit_unit()
    }

    serde_core::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char
        bytes byte_buf option newtype_struct seq tuple tuple_struct map struct enum identifier
    }
}

pub(crate) struct IvarsDeserializer<'de, 'a> {
    pub(crate) de: &'a mut Deserializer<'de>,
}

impl<'de> serde_core::de::Deserializer<'de> for IvarsDeserializer<'de, '_> {
    type Error = MarshalDeserializeError;

    fn deserialize_any<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.de.drive_ivar_map(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.deserialize_any(visitor)
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.deserialize_any(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.de.skip_ivars()?;
        visitor.visit_unit()
    }

    fn deserialize_unit<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.de.skip_ivars()?;
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.de.skip_ivars()?;
        visitor.visit_unit()
    }

    serde_core::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option newtype_struct seq tuple tuple_struct enum identifier
    }
}
