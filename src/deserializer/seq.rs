use serde_core::de::{DeserializeSeed, IntoDeserializer, SeqAccess, Visitor};

use super::{
    Deserializer, MarshalDeserializeError,
    map::{IvarsDeserializer, SymbolNameDeserializer},
    rb_str_to_str,
};
use crate::types::string::RbStr;

#[derive(Clone, Copy)]
enum SeqState {
    /// the first (useful) element is up next
    First,
    /// the second (extra information) element is up next
    Second,
    /// both elements have been served
    Done,
}

impl SeqState {
    fn remaining(self) -> usize {
        match self {
            SeqState::First => 2,
            SeqState::Second => 1,
            SeqState::Done => 0,
        }
    }
}

pub(crate) struct ArraySeqAccess<'de, 'a> {
    de: &'a mut Deserializer<'de>,
    remaining: usize,
}

impl<'de, 'a> ArraySeqAccess<'de, 'a> {
    pub(crate) fn new(de: &'a mut Deserializer<'de>, len: usize) -> Self {
        ArraySeqAccess { de, remaining: len }
    }

    pub(crate) fn finish(&mut self) -> Result<(), MarshalDeserializeError> {
        for _ in 0..std::mem::take(&mut self.remaining) {
            self.de.skip_value()?;
        }
        Ok(())
    }
}

impl<'de> SeqAccess<'de> for ArraySeqAccess<'de, '_> {
    type Error = MarshalDeserializeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, MarshalDeserializeError> {
        if self.remaining == 0 {
            return Ok(None);
        }
        self.remaining -= 1;
        seed.deserialize(&mut *self.de).map(Some)
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.remaining)
    }
}

pub(crate) struct InstanceSeqAccess<'de, 'a> {
    de: &'a mut Deserializer<'de>,
    state: SeqState,
}

impl<'de, 'a> InstanceSeqAccess<'de, 'a> {
    pub(crate) fn new(de: &'a mut Deserializer<'de>) -> Self {
        InstanceSeqAccess {
            de,
            state: SeqState::First,
        }
    }

    pub(crate) fn finish(&mut self) -> Result<(), MarshalDeserializeError> {
        match self.state {
            SeqState::First => {
                self.de.skip_value()?;
                self.de.skip_ivars()?;
            }
            SeqState::Second => self.de.skip_ivars()?,
            SeqState::Done => {}
        }
        self.state = SeqState::Done;
        Ok(())
    }
}

impl<'de> SeqAccess<'de> for InstanceSeqAccess<'de, '_> {
    type Error = MarshalDeserializeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, MarshalDeserializeError> {
        match self.state {
            SeqState::First => {
                self.state = SeqState::Second;
                seed.deserialize(&mut *self.de).map(Some)
            }
            SeqState::Second => {
                self.state = SeqState::Done;
                seed.deserialize(IvarsDeserializer { de: &mut *self.de })
                    .map(Some)
            }
            SeqState::Done => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.state.remaining())
    }
}

pub(crate) struct ObjectSeqAccess<'de, 'a> {
    de: &'a mut Deserializer<'de>,
    name: &'de str,
    state: SeqState,
}

impl<'de, 'a> ObjectSeqAccess<'de, 'a> {
    pub(crate) fn new(de: &'a mut Deserializer<'de>, name: &'de str) -> Self {
        ObjectSeqAccess {
            de,
            name,
            state: SeqState::First,
        }
    }

    pub(crate) fn finish(&mut self) -> Result<(), MarshalDeserializeError> {
        match self.state {
            SeqState::First => self.de.skip_ivars()?,
            SeqState::Second | SeqState::Done => {}
        }
        self.state = SeqState::Done;
        Ok(())
    }
}

impl<'de> SeqAccess<'de> for ObjectSeqAccess<'de, '_> {
    type Error = MarshalDeserializeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, MarshalDeserializeError> {
        match self.state {
            SeqState::First => {
                self.state = SeqState::Second;
                seed.deserialize(IvarsDeserializer { de: &mut *self.de })
                    .map(Some)
            }
            SeqState::Second => {
                self.state = SeqState::Done;
                seed.deserialize(SymbolNameDeserializer { name: self.name })
                    .map(Some)
            }
            SeqState::Done => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.state.remaining())
    }
}

pub(crate) struct ClassedSeqAccess<'de, 'a> {
    de: &'a mut Deserializer<'de>,
    name: &'de str,
    state: SeqState,
}

impl<'de, 'a> ClassedSeqAccess<'de, 'a> {
    pub(crate) fn new(de: &'a mut Deserializer<'de>, name: &'de str) -> Self {
        ClassedSeqAccess {
            de,
            name,
            state: SeqState::First,
        }
    }

    pub(crate) fn finish(&mut self) -> Result<(), MarshalDeserializeError> {
        match self.state {
            SeqState::First => self.de.skip_value()?,
            SeqState::Second | SeqState::Done => {}
        }
        self.state = SeqState::Done;
        Ok(())
    }
}

impl<'de> SeqAccess<'de> for ClassedSeqAccess<'de, '_> {
    type Error = MarshalDeserializeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, MarshalDeserializeError> {
        match self.state {
            SeqState::First => {
                self.state = SeqState::Second;
                seed.deserialize(&mut *self.de).map(Some)
            }
            SeqState::Second => {
                self.state = SeqState::Done;
                seed.deserialize(SymbolNameDeserializer { name: self.name })
                    .map(Some)
            }
            SeqState::Done => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.state.remaining())
    }
}

pub(crate) struct HashDefaultSeqAccess<'de, 'a> {
    de: &'a mut Deserializer<'de>,
    pairs: usize,
    state: SeqState,
}

impl<'de, 'a> HashDefaultSeqAccess<'de, 'a> {
    pub(crate) fn new(de: &'a mut Deserializer<'de>, pairs: usize) -> Self {
        HashDefaultSeqAccess {
            de,
            pairs,
            state: SeqState::First,
        }
    }

    pub(crate) fn finish(&mut self) -> Result<(), MarshalDeserializeError> {
        match self.state {
            SeqState::First => {
                self.de.skip_hash_pairs(self.pairs)?;
                self.de.skip_value()?;
            }
            SeqState::Second => self.de.skip_value()?,
            SeqState::Done => {}
        }
        self.state = SeqState::Done;
        Ok(())
    }
}

impl<'de> SeqAccess<'de> for HashDefaultSeqAccess<'de, '_> {
    type Error = MarshalDeserializeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, MarshalDeserializeError> {
        match self.state {
            SeqState::First => {
                self.state = SeqState::Second;
                seed.deserialize(HashPairsDeserializer {
                    de: &mut *self.de,
                    pairs: self.pairs,
                })
                .map(Some)
            }
            SeqState::Second => {
                self.state = SeqState::Done;
                seed.deserialize(&mut *self.de).map(Some)
            }
            SeqState::Done => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.state.remaining())
    }
}

pub(crate) struct UserDefinedSeqAccess<'de> {
    name: &'de str,
    payload: &'de [u8],
    state: SeqState,
}

impl<'de> UserDefinedSeqAccess<'de> {
    pub(crate) fn new(name: &'de str, payload: &'de [u8]) -> Self {
        UserDefinedSeqAccess {
            name,
            payload,
            state: SeqState::First,
        }
    }
}

impl<'de> SeqAccess<'de> for UserDefinedSeqAccess<'de> {
    type Error = MarshalDeserializeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, MarshalDeserializeError> {
        match self.state {
            SeqState::First => {
                self.state = SeqState::Second;
                seed.deserialize(serde_core::de::value::BorrowedBytesDeserializer::new(
                    self.payload,
                ))
                .map(Some)
            }
            SeqState::Second => {
                self.state = SeqState::Done;
                seed.deserialize(SymbolNameDeserializer { name: self.name })
                    .map(Some)
            }
            SeqState::Done => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.state.remaining())
    }
}

pub(crate) struct RegexSeqAccess<'a> {
    pattern: &'a RbStr,
    flags: u8,
    state: SeqState,
}

impl<'a> RegexSeqAccess<'a> {
    pub(crate) fn new(pattern: &'a RbStr, flags: u8) -> Self {
        RegexSeqAccess {
            pattern,
            flags,
            state: SeqState::First,
        }
    }
}

impl<'de> SeqAccess<'de> for RegexSeqAccess<'de> {
    type Error = MarshalDeserializeError;

    fn next_element_seed<T: DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, MarshalDeserializeError> {
        match self.state {
            SeqState::First => {
                self.state = SeqState::Second;
                let s = rb_str_to_str(self.pattern)?;
                seed.deserialize(serde_core::de::value::BorrowedStrDeserializer::new(s))
                    .map(Some)
            }
            SeqState::Second => {
                self.state = SeqState::Done;
                seed.deserialize(self.flags.into_deserializer()).map(Some)
            }
            SeqState::Done => Ok(None),
        }
    }

    fn size_hint(&self) -> Option<usize> {
        Some(self.state.remaining())
    }
}

pub(crate) struct HashPairsDeserializer<'de, 'a> {
    de: &'a mut Deserializer<'de>,
    pairs: usize,
}

impl<'de> serde_core::de::Deserializer<'de> for HashPairsDeserializer<'de, '_> {
    type Error = MarshalDeserializeError;

    fn deserialize_any<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.de.drive_hash(self.pairs, visitor)
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
        self.de.skip_hash_pairs(self.pairs)?;
        visitor.visit_unit()
    }

    fn deserialize_unit<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.de.skip_hash_pairs(self.pairs)?;
        visitor.visit_unit()
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.de.skip_hash_pairs(self.pairs)?;
        visitor.visit_unit()
    }

    serde_core::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option newtype_struct seq tuple tuple_struct enum identifier
    }
}
