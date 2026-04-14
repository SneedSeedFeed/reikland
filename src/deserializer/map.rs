use serde::de::{DeserializeSeed, MapAccess, Visitor};

use super::{Deserializer, Error, ErrorKind, rb_str_to_str};
use crate::{
    cursor::{object_table::ObjectIdx, symbol_table::SymbolIdx},
    marshal::MarshalData,
};
pub(crate) struct MapDeserializer<'a, 'b> {
    data: &'b MarshalData<'a>,
    iter: std::slice::Iter<'b, (ObjectIdx, ObjectIdx)>,
    value_idx: Option<ObjectIdx>,
}

impl<'a, 'b> MapDeserializer<'a, 'b> {
    pub(crate) fn new(data: &'b MarshalData<'a>, pairs: &'b [(ObjectIdx, ObjectIdx)]) -> Self {
        MapDeserializer {
            data,
            iter: pairs.iter(),
            value_idx: None,
        }
    }
}

impl<'de, 'b> MapAccess<'de> for MapDeserializer<'de, 'b> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        match self.iter.next() {
            Some(&(key_idx, val_idx)) => {
                self.value_idx = Some(val_idx);
                let de = Deserializer {
                    data: self.data,
                    idx: key_idx,
                };
                seed.deserialize(de).map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        let idx = self
            .value_idx
            .take()
            .expect("next_value_seed called before next_key_seed");
        let de = Deserializer {
            data: self.data,
            idx,
        };
        seed.deserialize(de)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

pub(crate) struct IvarMapDeserializer<'a, 'b> {
    data: &'b MarshalData<'a>,
    iter: std::slice::Iter<'b, (SymbolIdx, ObjectIdx)>,
    value_idx: Option<ObjectIdx>,
}

impl<'a, 'b> IvarMapDeserializer<'a, 'b> {
    pub(crate) fn new(data: &'b MarshalData<'a>, ivars: &'b [(SymbolIdx, ObjectIdx)]) -> Self {
        IvarMapDeserializer {
            data,
            iter: ivars.iter(),
            value_idx: None,
        }
    }
}

impl<'de, 'b> MapAccess<'de> for IvarMapDeserializer<'de, 'b> {
    type Error = Error;

    fn next_key_seed<K: DeserializeSeed<'de>>(
        &mut self,
        seed: K,
    ) -> Result<Option<K::Value>, Error> {
        match self.iter.next() {
            Some(&(sym_idx, val_idx)) => {
                self.value_idx = Some(val_idx);
                let rb = self
                    .data
                    .symbol(sym_idx)
                    .ok_or(ErrorKind::InvalidSymbolIndex(sym_idx.inner()))?;
                let s = rb_str_to_str(rb)?;
                seed.deserialize(serde::de::value::BorrowedStrDeserializer::new(s))
                    .map(Some)
            }
            None => Ok(None),
        }
    }

    fn next_value_seed<V: DeserializeSeed<'de>>(&mut self, seed: V) -> Result<V::Value, Error> {
        let idx = self
            .value_idx
            .take()
            .expect("next_value_seed called before next_key_seed");
        let de = Deserializer {
            data: self.data,
            idx,
        };
        seed.deserialize(de)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

pub(crate) struct SymbolNameDeserializer<'de> {
    pub(crate) name: &'de str,
}

impl<'de> serde::de::Deserializer<'de> for SymbolNameDeserializer<'de> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_borrowed_str(self.name)
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_any(visitor)
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_any(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char
        bytes byte_buf option newtype_struct seq tuple tuple_struct map struct enum identifier
    }
}

pub(crate) struct IvarsDeserializer<'a, 'b> {
    pub(crate) data: &'b MarshalData<'a>,
    pub(crate) ivars: &'b [(SymbolIdx, ObjectIdx)],
}

impl<'de, 'b> serde::de::Deserializer<'de> for IvarsDeserializer<'de, 'b> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_map(IvarMapDeserializer::new(self.data, self.ivars))
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_any(visitor)
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_any(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option newtype_struct seq tuple tuple_struct enum identifier
    }
}
