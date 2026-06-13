use serde_core::de::{DeserializeSeed, Visitor};

use super::{Deserializer, ErrorKind, MarshalDeserializeError, map::SymbolNameDeserializer};

pub(crate) struct UnitVariantDeserializer<'de> {
    pub(crate) name: &'de str,
}

impl<'de> serde_core::de::EnumAccess<'de> for UnitVariantDeserializer<'de> {
    type Error = MarshalDeserializeError;
    type Variant = UnitOnly;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), MarshalDeserializeError> {
        let val = seed.deserialize(SymbolNameDeserializer { name: self.name })?;
        Ok((val, UnitOnly))
    }
}

pub(crate) struct UnitOnly;

impl<'de> serde_core::de::VariantAccess<'de> for UnitOnly {
    type Error = MarshalDeserializeError;

    fn unit_variant(self) -> Result<(), MarshalDeserializeError> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(
        self,
        _seed: T,
    ) -> Result<T::Value, MarshalDeserializeError> {
        Err(ErrorKind::TypeMismatch {
            expected: "unit variant",
            got: "newtype variant",
        }
        .into())
    }

    fn tuple_variant<V: Visitor<'de>>(
        self,
        _: usize,
        _: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        Err(ErrorKind::TypeMismatch {
            expected: "unit variant",
            got: "tuple variant",
        }
        .into())
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _: &'static [&'static str],
        _: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        Err(ErrorKind::TypeMismatch {
            expected: "unit variant",
            got: "struct variant",
        }
        .into())
    }
}

pub(crate) struct MapVariantDeserializer<'de, 'a> {
    pub(crate) de: &'a mut Deserializer<'de>,
}

impl<'de, 'a> serde_core::de::EnumAccess<'de> for MapVariantDeserializer<'de, 'a> {
    type Error = MarshalDeserializeError;
    type Variant = MapVariantAccess<'de, 'a>;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), MarshalDeserializeError> {
        let val = seed.deserialize(&mut *self.de)?;
        Ok((val, MapVariantAccess { de: self.de }))
    }
}

pub(crate) struct MapVariantAccess<'de, 'a> {
    de: &'a mut Deserializer<'de>,
}

impl<'de> serde_core::de::VariantAccess<'de> for MapVariantAccess<'de, '_> {
    type Error = MarshalDeserializeError;

    fn unit_variant(self) -> Result<(), MarshalDeserializeError> {
        Err(ErrorKind::TypeMismatch {
            expected: "variant with data",
            got: "unit",
        }
        .into())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<T::Value, MarshalDeserializeError> {
        seed.deserialize(self.de)
    }

    fn tuple_variant<V: Visitor<'de>>(
        self,
        _: usize,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        serde_core::Deserializer::deserialize_seq(self.de, visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        serde_core::Deserializer::deserialize_map(self.de, visitor)
    }
}
