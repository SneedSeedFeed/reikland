use serde::de::Visitor;

use enum_access::{MapVariantDeserializer, UnitVariantDeserializer};

use map::MapDeserializer;
use seq::{
    ClassedSeqAccess, HashDefaultSeqAccess, InstanceSeqAccess, ObjectSeqAccess, RegexSeqAccess,
    SeqDeserializer, UserDefinedSeqAccess,
};

use crate::{
    cursor::{object_table::ObjectIdx, symbol_table::SymbolIdx},
    marshal::MarshalData,
    types::{string::RbStr, value::MarshalValue},
};

mod enum_access;
mod error;
mod map;
mod seq;

pub use error::Error;
pub(crate) use error::ErrorKind;
use error::type_mismatch;

/// Deserialize a `T` from raw Ruby Marshal bytes.
pub fn from_bytes<'a, T>(input: &'a [u8]) -> Result<T, Error>
where
    T: serde::de::Deserialize<'a>,
{
    let data = crate::marshal::parse(input)?;
    from_marshal_data(&data)
}

/// Deserialize a `T` from already-parsed [`MarshalData`].
pub fn from_marshal_data<'a, T>(data: &MarshalData<'a>) -> Result<T, Error>
where
    T: serde::de::Deserialize<'a>,
{
    let de = Deserializer {
        data,
        idx: data.root,
    };
    T::deserialize(de)
}

pub(crate) struct Deserializer<'a, 'b> {
    pub(crate) data: &'b MarshalData<'a>,
    pub(crate) idx: ObjectIdx,
}

/// Maximum depth for following [`MarshalValue::ObjectRef`] chains.
const MAX_REF_DEPTH: usize = 256;

// todo: smarter cycle detection than just running up to a limit?
impl<'a, 'b> Deserializer<'a, 'b> {
    /// Follow [`MarshalValue::ObjectRef`] chains until we reach a concrete value, up to [`MAX_REF_DEPTH`] times.
    fn resolve(&self, idx: ObjectIdx) -> Result<&'b MarshalValue<'a>, Error> {
        let mut val = self.data.object(idx);
        for _ in 0..MAX_REF_DEPTH {
            match val {
                MarshalValue::ObjectRef(ref_idx) => {
                    let obj_idx = self
                        .data
                        .objects
                        .get_by_ref(ref_idx.inner())
                        .expect("invalid object ref");
                    val = self.data.object(obj_idx);
                }
                _ => return Ok(val),
            }
        }
        Err(ErrorKind::CyclicRef.into())
    }

    /// Resolve a SymbolIdx to a UTF-8 string.
    fn symbol_str(&self, idx: SymbolIdx) -> Result<&'a str, Error> {
        let rb = self.resolve_symbol(idx)?;
        rb_str_to_str(rb)
    }

    /// Resolve the current value as a map
    fn resolve_as_hash(&self) -> Result<&'b [(ObjectIdx, ObjectIdx)], Error> {
        let val = self.resolve(self.idx)?;
        match val {
            MarshalValue::Hash(pairs) | MarshalValue::HashDefault { pairs, .. } => Ok(pairs),
            other => Err(type_mismatch("map", other)),
        }
    }

    fn resolve_symbol(&self, idx: SymbolIdx) -> Result<&'a RbStr, Error> {
        self.data
            .symbol(idx)
            .ok_or(ErrorKind::InvalidSymbolIndex(idx.inner()))
            .map_err(Error::from)
    }
}

pub(crate) fn rb_str_to_str(rb: &RbStr) -> Result<&str, Error> {
    rb.try_into()
        .map_err(ErrorKind::InvalidUtf8)
        .map_err(Error::from)
}

macro_rules! deserialize_int {
    ($self:ident, $visitor:ident, $method:ident, $visit:ident, $ty:ty) => {
        match $self.resolve($self.idx)? {
            MarshalValue::Fixnum(n) => {
                let v: $ty = (*n).try_into().map_err(|_| ErrorKind::IntegerOverflowI32 {
                    target_type: stringify!($ty),
                    value: *n,
                })?;
                $visitor.$visit(v)
            }
            MarshalValue::Bignum(big) => {
                let v: $ty = big
                    .try_into()
                    .map_err(|_| ErrorKind::IntegerOverflowBigInt {
                        target_type: stringify!($ty),
                        value: big.clone(),
                    })?;
                $visitor.$visit(v)
            }
            other => Err(type_mismatch("integer", other)),
        }
    };
}

impl<'de, 'b> serde::de::Deserializer<'de> for Deserializer<'de, 'b> {
    type Error = Error;

    fn deserialize_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.resolve(self.idx)? {
            MarshalValue::Nil => visitor.visit_unit(),
            MarshalValue::True => visitor.visit_bool(true),
            MarshalValue::False => visitor.visit_bool(false),
            MarshalValue::Fixnum(n) => visitor.visit_i32(*n),
            MarshalValue::Float(f) => visitor.visit_f64(*f),
            MarshalValue::Bignum(big) => {
                if let Ok(v) = i64::try_from(big) {
                    visitor.visit_i64(v)
                } else if let Ok(v) = u64::try_from(big) {
                    visitor.visit_u64(v)
                } else if let Ok(v) = i128::try_from(big) {
                    visitor.visit_i128(v)
                } else if let Ok(v) = u128::try_from(big) {
                    visitor.visit_u128(v)
                } else {
                    Err(ErrorKind::BignumTooLarge.into())
                }
            }
            MarshalValue::Class(rb)
            | MarshalValue::Module(rb)
            | MarshalValue::ClassOrModule(rb)
            | MarshalValue::Symbol(rb)
            | MarshalValue::String(rb) => match <&str>::try_from(*rb) {
                Ok(s) => visitor.visit_borrowed_str(s),
                Err(_) => visitor.visit_borrowed_bytes(rb.as_slice()),
            },
            MarshalValue::Regex { pattern, flags } => {
                visitor.visit_seq(RegexSeqAccess::new(pattern, *flags))
            }
            MarshalValue::SymbolLink(sym_idx) => {
                let symbol = self.resolve_symbol(*sym_idx)?;

                match symbol.try_into() {
                    Ok(utf8) => visitor.visit_borrowed_str(utf8),
                    Err(_) => visitor.visit_borrowed_bytes(symbol),
                }
            }
            MarshalValue::Array(elems) => visitor.visit_seq(SeqDeserializer::new(self.data, elems)),
            MarshalValue::Hash(pairs) => visitor.visit_map(MapDeserializer::new(self.data, pairs)),
            MarshalValue::HashDefault { pairs, default } => {
                visitor.visit_seq(HashDefaultSeqAccess::new(self.data, pairs, *default))
            }
            MarshalValue::Object { class, ivars }
            | MarshalValue::Struct {
                name: class,
                members: ivars,
            } => visitor.visit_seq(ObjectSeqAccess::new(self.data, *class, ivars)),
            MarshalValue::Instance { inner, ivars } => {
                visitor.visit_seq(InstanceSeqAccess::new(self.data, *inner, ivars))
            }
            MarshalValue::Extended { module, inner } => {
                visitor.visit_seq(ClassedSeqAccess::new(self.data, *module, *inner))
            }
            MarshalValue::UserMarshal { class, inner }
            | MarshalValue::UserString { class, inner }
            | MarshalValue::Data { class, inner } => {
                visitor.visit_seq(ClassedSeqAccess::new(self.data, *class, *inner))
            }
            MarshalValue::UserDefined { class, data } => {
                visitor.visit_seq(UserDefinedSeqAccess::new(self.data, *class, data))
            }
            other => Err(ErrorKind::UnsupportedType(other.as_snake_case()).into()),
        }
    }

    fn deserialize_bool<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.resolve(self.idx)? {
            MarshalValue::True => visitor.visit_bool(true),
            MarshalValue::False => visitor.visit_bool(false),
            other => Err(type_mismatch("bool", other)),
        }
    }

    fn deserialize_i8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        deserialize_int!(self, visitor, deserialize_i8, visit_i8, i8)
    }

    fn deserialize_i16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        deserialize_int!(self, visitor, deserialize_i16, visit_i16, i16)
    }

    fn deserialize_i32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        deserialize_int!(self, visitor, deserialize_i32, visit_i32, i32)
    }

    fn deserialize_i64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        deserialize_int!(self, visitor, deserialize_i64, visit_i64, i64)
    }

    fn deserialize_i128<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        deserialize_int!(self, visitor, deserialize_i128, visit_i128, i128)
    }

    fn deserialize_u8<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        deserialize_int!(self, visitor, deserialize_u8, visit_u8, u8)
    }

    fn deserialize_u16<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        deserialize_int!(self, visitor, deserialize_u16, visit_u16, u16)
    }

    fn deserialize_u32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        deserialize_int!(self, visitor, deserialize_u32, visit_u32, u32)
    }

    fn deserialize_u64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        deserialize_int!(self, visitor, deserialize_u64, visit_u64, u64)
    }

    fn deserialize_u128<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        deserialize_int!(self, visitor, deserialize_u128, visit_u128, u128)
    }

    fn deserialize_f32<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.resolve(self.idx)? {
            MarshalValue::Float(f) => visitor.visit_f32(*f as f32),
            MarshalValue::Fixnum(n) => visitor.visit_f32(*n as f32),
            other => Err(type_mismatch("float", other)),
        }
    }

    fn deserialize_f64<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.resolve(self.idx)? {
            MarshalValue::Float(f) => visitor.visit_f64(*f),
            MarshalValue::Fixnum(n) => visitor.visit_f64(*n as f64),
            other => Err(type_mismatch("float", other)),
        }
    }

    fn deserialize_char<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let val = self.resolve(self.idx)?;
        let rb = match val {
            MarshalValue::String(rb) | MarshalValue::Symbol(rb) => rb,
            MarshalValue::SymbolLink(sym_idx) => self.resolve_symbol(*sym_idx)?,
            other => {
                return Err(type_mismatch("char", other));
            }
        };
        let s = rb_str_to_str(rb)?;
        let mut chars = s.chars().collect::<Vec<_>>();
        if chars.len() != 1 {
            Err(ErrorKind::ExpectedSingleChar {
                len: s.chars().count(),
            }
            .into())
        } else {
            visitor.visit_char(chars.pop().unwrap())
        }
    }

    fn deserialize_str<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let val = self.resolve(self.idx)?;
        match val {
            MarshalValue::String(rb)
            | MarshalValue::Symbol(rb)
            | MarshalValue::Regex { pattern: rb, .. } => {
                let s = rb_str_to_str(rb)?;
                visitor.visit_borrowed_str(s)
            }
            MarshalValue::SymbolLink(sym_idx) => {
                let s = self.symbol_str(*sym_idx)?;
                visitor.visit_borrowed_str(s)
            }
            MarshalValue::Class(rb)
            | MarshalValue::Module(rb)
            | MarshalValue::ClassOrModule(rb) => {
                let s = rb_str_to_str(rb)?;
                visitor.visit_borrowed_str(s)
            }
            other => Err(type_mismatch("string", other)),
        }
    }

    fn deserialize_string<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let val = self.resolve(self.idx)?;
        match val {
            MarshalValue::String(rb) | MarshalValue::Symbol(rb) => {
                visitor.visit_borrowed_bytes(rb.as_slice())
            }
            MarshalValue::SymbolLink(sym_idx) => {
                let rb = self
                    .data
                    .symbol(*sym_idx)
                    .ok_or(ErrorKind::InvalidSymbolIndex(sym_idx.inner()))?;
                visitor.visit_borrowed_bytes(rb.as_slice())
            }
            MarshalValue::UserDefined { data, .. } => visitor.visit_borrowed_bytes(data),
            other => Err(type_mismatch("bytes", other)),
        }
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.resolve(self.idx)? {
            MarshalValue::Nil => visitor.visit_none(),
            _ => visitor.visit_some(self),
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        match self.resolve(self.idx)? {
            MarshalValue::Nil => visitor.visit_unit(),
            other => Err(type_mismatch("nil/unit", other)),
        }
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, Error> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let val = self.resolve(self.idx)?;
        match val {
            MarshalValue::Array(elems) => visitor.visit_seq(SeqDeserializer::new(self.data, elems)),
            MarshalValue::Object { class, ivars }
            | MarshalValue::Struct {
                name: class,
                members: ivars,
            } => visitor.visit_seq(ObjectSeqAccess::new(self.data, *class, ivars)),
            MarshalValue::Instance { inner, ivars } => {
                visitor.visit_seq(InstanceSeqAccess::new(self.data, *inner, ivars))
            }
            MarshalValue::Extended { module, inner } => {
                visitor.visit_seq(ClassedSeqAccess::new(self.data, *module, *inner))
            }
            MarshalValue::UserMarshal { class, inner }
            | MarshalValue::UserString { class, inner }
            | MarshalValue::Data { class, inner } => {
                visitor.visit_seq(ClassedSeqAccess::new(self.data, *class, *inner))
            }
            MarshalValue::UserDefined { class, data } => {
                visitor.visit_seq(UserDefinedSeqAccess::new(self.data, *class, data))
            }
            MarshalValue::Regex { pattern, flags } => {
                visitor.visit_seq(RegexSeqAccess::new(pattern, *flags))
            }
            MarshalValue::HashDefault { pairs, default } => {
                visitor.visit_seq(HashDefaultSeqAccess::new(self.data, pairs, *default))
            }
            other => Err(type_mismatch("sequence", other)),
        }
    }

    fn deserialize_tuple<V: Visitor<'de>>(self, _: usize, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: usize,
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        let pairs = self.resolve_as_hash()?;
        visitor.visit_map(MapDeserializer::new(self.data, pairs))
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, Error> {
        let val = self.resolve(self.idx)?;
        match val {
            MarshalValue::Symbol(_) | MarshalValue::SymbolLink(_) | MarshalValue::String(_) => {
                visitor.visit_enum(UnitVariantDeserializer {
                    de: Deserializer {
                        data: self.data,
                        idx: self.idx,
                    },
                })
            }
            MarshalValue::Hash(pairs) if pairs.len() == 1 => {
                let (key_idx, val_idx) = pairs[0];
                visitor.visit_enum(MapVariantDeserializer {
                    data: self.data,
                    key_idx,
                    val_idx,
                })
            }
            other => Err(type_mismatch("enum (symbol or single-entry hash)", other)),
        }
    }

    fn deserialize_identifier<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(self, visitor: V) -> Result<V::Value, Error> {
        visitor.visit_unit()
    }
}

// vast majority of tests are AI generated, I probably wouldn't do it otherwise.
#[cfg(test)]
mod tests {

    use serde::Deserialize;
    use std::collections::HashMap;

    use crate::{
        deserializer::{Error, from_marshal_data},
        deserializer_types::{
            ivar::{Ivar, WithEncoding},
            rb_object::RbObject,
        },
        marshal::{self, MarshalData},
        types::encoding::RubyEncoding,
    };

    // Helper to parse and deserialize
    fn de_from_ruby<'a, T: Deserialize<'a>>(data: &'a MarshalData<'a>) -> Result<T, Error> {
        from_marshal_data(data)
    }

    #[test]
    fn test_nil_to_unit() {
        // Marshal.dump(nil) = \x04\x080
        let bytes = b"\x04\x080";
        let data = marshal::parse(bytes).unwrap();
        let result: () = de_from_ruby(&data).unwrap();
        assert_eq!(result, ());
    }

    #[test]
    fn test_nil_to_option() {
        let bytes = b"\x04\x080";
        let data = marshal::parse(bytes).unwrap();
        let result: Option<i32> = de_from_ruby(&data).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_bool_true() {
        // Marshal.dump(true) = \x04\x08T
        let bytes = b"\x04\x08T";
        let data = marshal::parse(bytes).unwrap();
        let result: bool = de_from_ruby(&data).unwrap();
        assert!(result);
    }

    #[test]
    fn test_bool_false() {
        let bytes = b"\x04\x08F";
        let data = marshal::parse(bytes).unwrap();
        let result: bool = de_from_ruby(&data).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_fixnum_zero() {
        // Marshal.dump(0) = \x04\x08i\x00
        let bytes = b"\x04\x08i\x00";
        let data = marshal::parse(bytes).unwrap();
        let result: i32 = de_from_ruby(&data).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_fixnum_positive() {
        // Marshal.dump(42) = \x04\x08i/
        let bytes = b"\x04\x08i/";
        let data = marshal::parse(bytes).unwrap();
        let result: i32 = de_from_ruby(&data).unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn test_fixnum_to_option_some() {
        let bytes = b"\x04\x08i/";
        let data = marshal::parse(bytes).unwrap();
        let result: Option<i32> = de_from_ruby(&data).unwrap();
        assert_eq!(result, Some(42));
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_float() {
        // Marshal.dump(3.14) = \x04\x08f\x093.14
        let bytes = b"\x04\x08f\x093.14";
        let data = marshal::parse(bytes).unwrap();
        let result: f64 = de_from_ruby(&data).unwrap();
        assert!((result - 3.14).abs() < f64::EPSILON);
    }

    #[test]
    fn test_array_of_ints() {
        // Marshal.dump([1,2,3]) = \x04\x08[\x08i\x06i\x07i\x08
        let bytes = b"\x04\x08[\x08i\x06i\x07i\x08";
        let data = marshal::parse(bytes).unwrap();
        let result: Vec<i32> = de_from_ruby(&data).unwrap();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn test_symbol() {
        // Marshal.dump(:hello) = \x04\x08:\x0ahello
        let bytes = b"\x04\x08:\x0ahello";
        let data = marshal::parse(bytes).unwrap();
        let result: &str = de_from_ruby(&data).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_string_with_instance_wrapper() {
        // Marshal.dump("hello") = \x04\x08I\"\x0ahello\x06:\x06ET
        // Instance wrapping is now explicit - use Ivar<T> to unwrap
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let data = marshal::parse(bytes).unwrap();
        let result: Ivar<&str> = de_from_ruby(&data).unwrap();
        assert_eq!(result.inner, "hello");
    }

    #[test]
    fn test_string_ivar_with_ivars_captured() {
        // Same data, but capture the encoding ivar
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let data = marshal::parse(bytes).unwrap();
        let result: Ivar<&str, HashMap<&str, bool>> = de_from_ruby(&data).unwrap();
        assert_eq!(result.inner, "hello");
        assert_eq!(result.ivars.get("E"), Some(&true));
    }

    #[test]
    fn test_instance_as_tuple() {
        // Instance can also be deserialized as a raw tuple
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let data = marshal::parse(bytes).unwrap();
        let result: (&str, HashMap<&str, bool>) = de_from_ruby(&data).unwrap();
        assert_eq!(result.0, "hello");
        assert_eq!(result.1.get("E"), Some(&true));
    }

    #[test]
    fn test_hash_to_hashmap() {
        // Marshal.dump({a: 1, b: 2}) = hash with symbol keys
        // \x04\x08{\x07:\x06ai\x06:\x06bi\x07
        let bytes = b"\x04\x08{\x07:\x06ai\x06:\x06bi\x07";
        let data = marshal::parse(bytes).unwrap();
        let result: HashMap<&str, i32> = de_from_ruby(&data).unwrap();
        assert_eq!(result.get("a"), Some(&1));
        assert_eq!(result.get("b"), Some(&2));
    }

    #[test]
    fn test_hash_to_struct() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Point {
            x: i32,
            y: i32,
        }

        // Marshal.dump({x: 10, y: 20})
        // \x04\x08{\x07:\x06xi\x0f:\x06yi\x19
        let bytes = b"\x04\x08{\x07:\x06xi\x0f:\x06yi\x19";
        let data = marshal::parse(bytes).unwrap();
        let result: Point = de_from_ruby(&data).unwrap();
        assert_eq!(result, Point { x: 10, y: 20 });
    }

    #[test]
    fn test_nested_array() {
        // Marshal.dump([1, [2, 3]])
        let bytes = b"\x04\x08[\x07i\x06[\x07i\x07i\x08";
        let data = marshal::parse(bytes).unwrap();
        let result: (i32, Vec<i32>) = de_from_ruby(&data).unwrap();
        assert_eq!(result, (1, vec![2, 3]));
    }

    // ---- Ivar deserialization tests ----

    #[test]
    fn ivar_discard_ivars() {
        // Marshal.dump("hello") = I"\x0ahello\x06:\x06ET
        // Ivar<T> (O=()) discards the encoding ivar
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let data = marshal::parse(bytes).unwrap();
        let result: Ivar<&str> = de_from_ruby(&data).unwrap();
        assert_eq!(result.inner, "hello");
        assert_eq!(result.ivars, ());
    }

    #[test]
    fn ivar_capture_as_hashmap() {
        // Capture ivars into a HashMap
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let data = marshal::parse(bytes).unwrap();
        let result: Ivar<&str, HashMap<&str, bool>> = de_from_ruby(&data).unwrap();
        assert_eq!(result.inner, "hello");
        assert_eq!(result.ivars.len(), 1);
        assert!(result.ivars["E"]);
    }

    #[test]
    fn ivar_as_raw_tuple() {
        // Instance can be deserialized as a plain tuple
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let data = marshal::parse(bytes).unwrap();
        let result: (&str, HashMap<&str, bool>) = de_from_ruby(&data).unwrap();
        assert_eq!(result.0, "hello");
        assert!(result.1["E"]);
    }

    #[test]
    fn ivar_deref_to_inner() {
        // Ivar<T> derefs to T
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let data = marshal::parse(bytes).unwrap();
        let result: Ivar<&str> = de_from_ruby(&data).unwrap();
        let s: &str = &result;
        assert_eq!(s, "hello");
    }

    #[test]
    fn ivar_utf8_encoding() {
        // E: true → UTF-8
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let data = marshal::parse(bytes).unwrap();
        let result: WithEncoding<&str> = de_from_ruby(&data).unwrap();
        assert_eq!(result.inner, "hello");
        assert_eq!(*result.ivars, RubyEncoding::Utf8);
    }

    #[test]
    fn ivar_ascii_encoding() {
        // E: false → US-ASCII
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06EF";
        let data = marshal::parse(bytes).unwrap();
        let result: WithEncoding<&str> = de_from_ruby(&data).unwrap();
        assert_eq!(result.inner, "hello");
        assert_eq!(*result.ivars, RubyEncoding::UsAscii);
    }

    #[test]
    fn ivar_explicit_encoding() {
        // encoding: "Shift_JIS" — uses the :encoding ivar instead of :E
        // Marshal.dump("hello".encode("Shift_JIS"))
        // I"\x0ahello\x06:\x0dencoding"\x0eShift_JIS
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x0dencoding\"\x0eShift_JIS";
        let data = marshal::parse(bytes).unwrap();
        let result: WithEncoding<&str> = de_from_ruby(&data).unwrap();
        assert_eq!(result.inner, "hello");
        assert_eq!(*result.ivars, RubyEncoding::ShiftJis);
    }

    #[test]
    fn ivar_encoding_deref() {
        // Encoding derefs to RubyEncoding
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let data = marshal::parse(bytes).unwrap();
        let result: WithEncoding<&str> = de_from_ruby(&data).unwrap();
        let enc: &RubyEncoding = &result.ivars;
        assert_eq!(*enc, RubyEncoding::Utf8);
    }

    #[test]
    fn ivar_multiple_ivars() {
        // Instance with 2 ivars: E: true and another custom one
        // I"\x0ahello\x07:\x06ET:\x06xi\x2a  (E => true, x => 37)
        let bytes = b"\x04\x08I\"\x0ahello\x07:\x06ET:\x06xi\x2a";
        let data = marshal::parse(bytes).unwrap();
        // Capture all ivars as a struct
        #[derive(Debug, Deserialize)]
        struct Meta {
            #[serde(rename = "E")]
            encoding: bool,
            x: i32,
        }
        let result: Ivar<&str, Meta> = de_from_ruby(&data).unwrap();
        assert_eq!(result.inner, "hello");
        assert!(result.ivars.encoding);
        assert_eq!(result.ivars.x, 37);
    }

    #[test]
    fn ivar_encoding_ignores_extra_ivars() {
        // Encoding skips unknown ivars
        // I"\x0ahello\x07:\x06ET:\x06xi\x2a
        let bytes = b"\x04\x08I\"\x0ahello\x07:\x06ET:\x06xi\x2a";
        let data = marshal::parse(bytes).unwrap();
        let result: WithEncoding<&str> = de_from_ruby(&data).unwrap();
        assert_eq!(result.inner, "hello");
        assert_eq!(*result.ivars, RubyEncoding::Utf8);
    }

    #[test]
    fn ivar_in_array() {
        // Array of Instance-wrapped strings: ["hello", "world"]
        // [\x07 I"\x0ahello\x06:\x06ET I"\x0aworld\x06:\x06ET
        let bytes = b"\x04\x08[\x07I\"\x0ahello\x06:\x06ETI\"\x0aworld\x06:\x06ET";
        let data = marshal::parse(bytes).unwrap();
        let result: Vec<Ivar<&str>> = de_from_ruby(&data).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].inner, "hello");
        assert_eq!(result[1].inner, "world");
    }

    #[test]
    fn ivar_string_not_transparent() {
        // Deserializing an Instance-wrapped string directly as &str should fail
        // because Instance is a sequence, not a string
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let data = marshal::parse(bytes).unwrap();
        let result: Result<&str, Error> = de_from_ruby(&data);
        assert!(result.is_err());
    }

    #[test]
    fn ivar_with_encoding_in_array() {
        // Array of WithEncoding strings
        let bytes = b"\x04\x08[\x07I\"\x0ahello\x06:\x06ETI\"\x0aworld\x06:\x06EF";
        let data = marshal::parse(bytes).unwrap();
        let result: Vec<WithEncoding<&str>> = de_from_ruby(&data).unwrap();
        assert_eq!(result[0].inner, "hello");
        assert_eq!(*result[0].ivars, RubyEncoding::Utf8);
        assert_eq!(result[1].inner, "world");
        assert_eq!(*result[1].ivars, RubyEncoding::UsAscii);
    }

    // ---- Object / Struct deserialization tests ----

    // Marshal.dump(Pt.new.tap { |p| p.x = 10; p.y = 20 })
    // where: class Pt; attr_accessor :x, :y; end
    // o:\x07Pt\x07:\x07@xi\x0f:\x07@yi\x19
    const OBJECT_PT: &[u8] = b"\x04\x08o:\x07Pt\x07:\x07@xi\x0f:\x07@yi\x19";

    // Marshal.dump(Pt.new(10, 20))
    // where: Pt = Struct.new(:x, :y)
    // S:\x07Pt\x07:\x06xi\x0f:\x06yi\x19
    const STRUCT_PT: &[u8] = b"\x04\x08S:\x07Pt\x07:\x06xi\x0f:\x06yi\x19";

    #[test]
    fn object_discard_class() {
        // RbObject<T> (N=()) discards the class name
        #[derive(Debug, Deserialize, PartialEq)]
        struct Pt {
            #[serde(rename = "@x")]
            x: i32,
            #[serde(rename = "@y")]
            y: i32,
        }
        let data = marshal::parse(OBJECT_PT).unwrap();
        let result: RbObject<Pt> = de_from_ruby(&data).unwrap();
        assert_eq!(result.fields, Pt { x: 10, y: 20 });
        assert_eq!(result.class, ());
    }

    #[test]
    fn object_capture_class() {
        // Capture the class name as &str
        #[derive(Debug, Deserialize, PartialEq)]
        struct Pt {
            #[serde(rename = "@x")]
            x: i32,
            #[serde(rename = "@y")]
            y: i32,
        }
        let data = marshal::parse(OBJECT_PT).unwrap();
        let result: RbObject<Pt, &str> = de_from_ruby(&data).unwrap();
        assert_eq!(result.class, "Pt");
        assert_eq!(result.fields, Pt { x: 10, y: 20 });
    }

    #[test]
    fn object_as_raw_tuple() {
        // Object can be deserialized as a plain tuple
        let data = marshal::parse(OBJECT_PT).unwrap();
        let result: (&str, HashMap<&str, i32>) = de_from_ruby(&data).unwrap();
        assert_eq!(result.0, "Pt");
        assert_eq!(result.1["@x"], 10);
        assert_eq!(result.1["@y"], 20);
    }

    #[test]
    fn object_deref_to_fields() {
        // RbObject<T> derefs to T
        #[derive(Debug, Deserialize)]
        struct Pt {
            #[serde(rename = "@x")]
            x: i32,
        }
        let data = marshal::parse(OBJECT_PT).unwrap();
        let result: RbObject<Pt> = de_from_ruby(&data).unwrap();
        assert_eq!(result.x, 10); // accessed through Deref
    }

    #[test]
    fn object_not_transparent() {
        // Deserializing an Object directly as a struct (without RbObject) should fail
        // because Object is now a sequence, not a map
        #[derive(Debug, Deserialize)]
        #[allow(dead_code)]
        struct Pt {
            #[serde(rename = "@x")]
            x: i32,
        }
        let data = marshal::parse(OBJECT_PT).unwrap();
        let result: Result<Pt, Error> = de_from_ruby(&data);
        assert!(result.is_err());
    }

    #[test]
    fn object_fields_as_hashmap() {
        // Fields can be captured as a HashMap
        let data = marshal::parse(OBJECT_PT).unwrap();
        let result: RbObject<HashMap<&str, i32>> = de_from_ruby(&data).unwrap();
        assert_eq!(result.fields["@x"], 10);
        assert_eq!(result.fields["@y"], 20);
    }

    #[test]
    fn struct_discard_name() {
        // Ruby Struct works the same as Object — RbStruct is just an alias
        #[derive(Debug, Deserialize, PartialEq)]
        struct Pt {
            x: i32,
            y: i32,
        }
        let data = marshal::parse(STRUCT_PT).unwrap();
        let result: RbObject<Pt> = de_from_ruby(&data).unwrap();
        assert_eq!(result.fields, Pt { x: 10, y: 20 });
    }

    #[test]
    fn struct_capture_name() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Pt {
            x: i32,
            y: i32,
        }
        let data = marshal::parse(STRUCT_PT).unwrap();
        let result: RbObject<Pt, &str> = de_from_ruby(&data).unwrap();
        assert_eq!(result.class, "Pt");
        assert_eq!(result.fields, Pt { x: 10, y: 20 });
    }

    #[test]
    fn struct_as_raw_tuple() {
        let data = marshal::parse(STRUCT_PT).unwrap();
        let result: (&str, HashMap<&str, i32>) = de_from_ruby(&data).unwrap();
        assert_eq!(result.0, "Pt");
        assert_eq!(result.1["x"], 10);
        assert_eq!(result.1["y"], 20);
    }

    #[test]
    fn object_in_array() {
        // Array of two Pt Objects: {x:1, y:2} and {x:3, y:4}
        let bytes = b"\x04\x08[\x07o:\x07Pt\x07:\x07@xi\x06:\x07@yi\x07o:\x07Pt\x07:\x07@xi\x08:\x07@yi\x09";
        let data = marshal::parse(bytes).unwrap();
        #[derive(Debug, Deserialize, PartialEq)]
        struct Pt {
            #[serde(rename = "@x")]
            x: i32,
            #[serde(rename = "@y")]
            y: i32,
        }
        let result: Vec<RbObject<Pt>> = de_from_ruby(&data).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].fields, Pt { x: 1, y: 2 });
        assert_eq!(result[1].fields, Pt { x: 3, y: 4 });
    }
}
