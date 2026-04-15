use serde::de::{DeserializeSeed, IntoDeserializer, SeqAccess};

use super::map::{IvarsDeserializer, SymbolNameDeserializer};
use super::{Deserializer, ErrorKind, rb_str_to_str};
use crate::cursor::symbol_table::SymbolIdx;
use crate::types::string::RbStr;
use crate::{cursor::object_table::ObjectIdx, marshal::MarshalData};

pub(crate) struct SeqDeserializer<'a, 'b> {
    data: &'b MarshalData<'a>,
    iter: std::slice::Iter<'b, ObjectIdx>,
}

impl<'a, 'b> SeqDeserializer<'a, 'b> {
    pub(crate) fn new(data: &'b MarshalData<'a>, elems: &'b [ObjectIdx]) -> Self {
        SeqDeserializer {
            data,
            iter: elems.iter(),
        }
    }
}

impl<'de, 'b> SeqAccess<'de> for SeqDeserializer<'de, 'b> {
    type Error = super::MarshalDeserializeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, super::MarshalDeserializeError> {
        match self.iter.next() {
            Some(&idx) => {
                let de = Deserializer {
                    data: self.data,
                    idx,
                };
                seed.deserialize(de).map(Some)
            }
            None => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.iter.len())
    }
}

pub(crate) struct InstanceSeqAccess<'a, 'b> {
    data: &'b MarshalData<'a>,
    inner: ObjectIdx,
    ivars: &'b [(SymbolIdx, ObjectIdx)],
    state: u8,
}

impl<'a, 'b> InstanceSeqAccess<'a, 'b> {
    pub(crate) fn new(
        data: &'b MarshalData<'a>,
        inner: ObjectIdx,
        ivars: &'b [(SymbolIdx, ObjectIdx)],
    ) -> Self {
        InstanceSeqAccess {
            data,
            inner,
            ivars,
            state: 0,
        }
    }
}

impl<'de, 'b> SeqAccess<'de> for InstanceSeqAccess<'de, 'b> {
    type Error = super::MarshalDeserializeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, super::MarshalDeserializeError> {
        match self.state {
            0 => {
                self.state = 1;
                let de = Deserializer {
                    data: self.data,
                    idx: self.inner,
                };
                seed.deserialize(de).map(Some)
            }
            1 => {
                self.state = 2;
                let de = IvarsDeserializer {
                    data: self.data,
                    ivars: self.ivars,
                };
                seed.deserialize(de).map(Some)
            }
            _ => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some((2 - self.state as usize).min(2))
    }
}

pub(crate) struct ClassedSeqAccess<'a, 'b> {
    data: &'b MarshalData<'a>,
    name: SymbolIdx,
    inner: ObjectIdx,
    state: u8,
}

impl<'a, 'b> ClassedSeqAccess<'a, 'b> {
    pub(crate) fn new(data: &'b MarshalData<'a>, name: SymbolIdx, inner: ObjectIdx) -> Self {
        ClassedSeqAccess {
            data,
            name,
            inner,
            state: 0,
        }
    }
}

impl<'de, 'b> SeqAccess<'de> for ClassedSeqAccess<'de, 'b> {
    type Error = super::MarshalDeserializeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, super::MarshalDeserializeError> {
        match self.state {
            0 => {
                self.state = 1;
                let rb = self
                    .data
                    .symbol(self.name)
                    .ok_or(ErrorKind::InvalidSymbolIndex(self.name.inner()))?;
                let s = rb_str_to_str(rb)?;
                seed.deserialize(SymbolNameDeserializer { name: s })
                    .map(Some)
            }
            1 => {
                self.state = 2;
                let de = Deserializer {
                    data: self.data,
                    idx: self.inner,
                };
                seed.deserialize(de).map(Some)
            }
            _ => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some((2 - self.state as usize).min(2))
    }
}

pub(crate) struct ObjectSeqAccess<'a, 'b> {
    data: &'b MarshalData<'a>,
    name: SymbolIdx,
    ivars: &'b [(SymbolIdx, ObjectIdx)],
    state: u8,
}

impl<'a, 'b> ObjectSeqAccess<'a, 'b> {
    pub(crate) fn new(
        data: &'b MarshalData<'a>,
        name: SymbolIdx,
        ivars: &'b [(SymbolIdx, ObjectIdx)],
    ) -> Self {
        ObjectSeqAccess {
            data,
            name,
            ivars,
            state: 0,
        }
    }
}

impl<'de, 'b> SeqAccess<'de> for ObjectSeqAccess<'de, 'b> {
    type Error = super::MarshalDeserializeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, super::MarshalDeserializeError> {
        match self.state {
            0 => {
                self.state = 1;
                let rb = self
                    .data
                    .symbol(self.name)
                    .ok_or(ErrorKind::InvalidSymbolIndex(self.name.inner()))?;
                let s = rb_str_to_str(rb)?;
                seed.deserialize(SymbolNameDeserializer { name: s })
                    .map(Some)
            }
            1 => {
                self.state = 2;
                let de = IvarsDeserializer {
                    data: self.data,
                    ivars: self.ivars,
                };
                seed.deserialize(de).map(Some)
            }
            _ => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some((2 - self.state as usize).min(2))
    }
}

pub(crate) struct UserDefinedSeqAccess<'a, 'b> {
    data: &'b MarshalData<'a>,
    class: SymbolIdx,
    payload: &'a [u8],
    state: u8,
}

impl<'a, 'b> UserDefinedSeqAccess<'a, 'b> {
    pub(crate) fn new(data: &'b MarshalData<'a>, class: SymbolIdx, payload: &'a [u8]) -> Self {
        UserDefinedSeqAccess {
            data,
            class,
            payload,
            state: 0,
        }
    }
}

impl<'de, 'b> SeqAccess<'de> for UserDefinedSeqAccess<'de, 'b> {
    type Error = super::MarshalDeserializeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, super::MarshalDeserializeError> {
        match self.state {
            0 => {
                self.state = 1;
                let rb = self
                    .data
                    .symbol(self.class)
                    .ok_or(ErrorKind::InvalidSymbolIndex(self.class.inner()))?;
                let s = rb_str_to_str(rb)?;
                seed.deserialize(SymbolNameDeserializer { name: s })
                    .map(Some)
            }
            1 => {
                self.state = 2;
                seed.deserialize(serde::de::value::BorrowedBytesDeserializer::new(
                    self.payload,
                ))
                .map(Some)
            }
            _ => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some((2 - self.state as usize).min(2))
    }
}

pub(crate) struct RegexSeqAccess<'a> {
    pattern: &'a RbStr,
    flags: u8,
    state: u8,
}

impl<'a> RegexSeqAccess<'a> {
    pub(crate) fn new(pattern: &'a RbStr, flags: u8) -> Self {
        RegexSeqAccess {
            pattern,
            flags,
            state: 0,
        }
    }
}

impl<'de> SeqAccess<'de> for RegexSeqAccess<'de> {
    type Error = super::MarshalDeserializeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, super::MarshalDeserializeError> {
        match self.state {
            0 => {
                self.state = 1;
                let s = rb_str_to_str(self.pattern)?;
                seed.deserialize(serde::de::value::BorrowedStrDeserializer::new(s))
                    .map(Some)
            }
            1 => {
                self.state = 2;
                seed.deserialize(self.flags.into_deserializer()).map(Some)
            }
            _ => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some((2 - self.state as usize).min(2))
    }
}

/// Sequence access for HashDefault: yields `(hash_map, default_value)`.
pub(crate) struct HashDefaultSeqAccess<'a, 'b> {
    data: &'b MarshalData<'a>,
    pairs: &'b [(ObjectIdx, ObjectIdx)],
    default: ObjectIdx,
    state: u8,
}

impl<'a, 'b> HashDefaultSeqAccess<'a, 'b> {
    pub(crate) fn new(
        data: &'b MarshalData<'a>,
        pairs: &'b [(ObjectIdx, ObjectIdx)],
        default: ObjectIdx,
    ) -> Self {
        HashDefaultSeqAccess {
            data,
            pairs,
            default,
            state: 0,
        }
    }
}

impl<'de, 'b> SeqAccess<'de> for HashDefaultSeqAccess<'de, 'b> {
    type Error = super::MarshalDeserializeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, super::MarshalDeserializeError> {
        match self.state {
            0 => {
                self.state = 1;
                let de = HashPairsDeserializer {
                    data: self.data,
                    pairs: self.pairs,
                };
                seed.deserialize(de).map(Some)
            }
            1 => {
                self.state = 2;
                let de = Deserializer {
                    data: self.data,
                    idx: self.default,
                };
                seed.deserialize(de).map(Some)
            }
            _ => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some((2 - self.state as usize).min(2))
    }
}

pub(crate) struct HashPairsDeserializer<'a, 'b> {
    data: &'b MarshalData<'a>,
    pairs: &'b [(ObjectIdx, ObjectIdx)],
}

impl<'de, 'b> serde::de::Deserializer<'de> for HashPairsDeserializer<'de, 'b> {
    type Error = super::MarshalDeserializeError;

    fn deserialize_any<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, super::MarshalDeserializeError> {
        visitor.visit_map(super::map::MapDeserializer::new(self.data, self.pairs))
    }

    fn deserialize_map<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, super::MarshalDeserializeError> {
        self.deserialize_any(visitor)
    }

    fn deserialize_struct<V: serde::de::Visitor<'de>>(
        self,
        _name: &'static str,
        _fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, super::MarshalDeserializeError> {
        self.deserialize_any(visitor)
    }

    fn deserialize_ignored_any<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, super::MarshalDeserializeError> {
        visitor.visit_unit()
    }

    fn deserialize_unit<V: serde::de::Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, super::MarshalDeserializeError> {
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: serde::de::Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, super::MarshalDeserializeError> {
        visitor.visit_unit()
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option newtype_struct seq tuple tuple_struct enum identifier
    }
}
