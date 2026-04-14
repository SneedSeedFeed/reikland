use serde::de::{DeserializeSeed, Visitor};

use super::{Deserializer, Error, ErrorKind};
use crate::{cursor::object_table::ObjectIdx, marshal::MarshalData};

pub(crate) struct UnitVariantDeserializer<'a, 'b> {
    pub(crate) de: Deserializer<'a, 'b>,
}

impl<'de, 'b> serde::de::EnumAccess<'de> for UnitVariantDeserializer<'de, 'b> {
    type Error = Error;
    type Variant = UnitOnly;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Error> {
        let val = seed.deserialize(self.de)?;
        Ok((val, UnitOnly))
    }
}

pub(crate) struct UnitOnly;

impl<'de> serde::de::VariantAccess<'de> for UnitOnly {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        Ok(())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, _seed: T) -> Result<T::Value, Error> {
        Err(ErrorKind::TypeMismatch {
            expected: "unit variant",
            got: "newtype variant",
        }
        .into())
    }

    fn tuple_variant<V: Visitor<'de>>(self, _: usize, _: V) -> Result<V::Value, Error> {
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
    ) -> Result<V::Value, Error> {
        Err(ErrorKind::TypeMismatch {
            expected: "unit variant",
            got: "struct variant",
        }
        .into())
    }
}

pub(crate) struct MapVariantDeserializer<'a, 'b> {
    pub(crate) data: &'b MarshalData<'a>,
    pub(crate) key_idx: ObjectIdx,
    pub(crate) val_idx: ObjectIdx,
}

impl<'de, 'b> serde::de::EnumAccess<'de> for MapVariantDeserializer<'de, 'b> {
    type Error = Error;
    type Variant = MapVariantAccess<'de, 'b>;

    fn variant_seed<V: DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, Self::Variant), Error> {
        let key_de = Deserializer {
            data: self.data,
            idx: self.key_idx,
        };
        let val = seed.deserialize(key_de)?;
        Ok((
            val,
            MapVariantAccess {
                data: self.data,
                val_idx: self.val_idx,
            },
        ))
    }
}

pub(crate) struct MapVariantAccess<'a, 'b> {
    data: &'b MarshalData<'a>,
    val_idx: ObjectIdx,
}

impl<'de, 'b> serde::de::VariantAccess<'de> for MapVariantAccess<'de, 'b> {
    type Error = Error;

    fn unit_variant(self) -> Result<(), Error> {
        Err(ErrorKind::TypeMismatch {
            expected: "variant with data",
            got: "unit",
        }
        .into())
    }

    fn newtype_variant_seed<T: DeserializeSeed<'de>>(self, seed: T) -> Result<T::Value, Error> {
        let de = Deserializer {
            data: self.data,
            idx: self.val_idx,
        };
        seed.deserialize(de)
    }

    fn tuple_variant<V: Visitor<'de>>(self, _: usize, visitor: V) -> Result<V::Value, Error> {
        let de = Deserializer {
            data: self.data,
            idx: self.val_idx,
        };
        serde::Deserializer::deserialize_seq(de, visitor)
    }

    fn struct_variant<V: Visitor<'de>>(
        self,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        let de = Deserializer {
            data: self.data,
            idx: self.val_idx,
        };
        serde::Deserializer::deserialize_map(de, visitor)
    }
}
