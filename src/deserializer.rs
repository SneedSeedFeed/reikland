use std::num::NonZeroUsize;

use enum_access::{MapVariantDeserializer, UnitVariantDeserializer};
use map::{HashMapAccess, IvarMapAccess};
use num_bigint::BigInt;
use seq::{
    ArraySeqAccess, ClassedSeqAccess, HashDefaultSeqAccess, InstanceSeqAccess, ObjectSeqAccess,
    RegexSeqAccess, UserDefinedSeqAccess,
};
use serde_core::de::Visitor;

use crate::{
    cursor::{Cursor, FromCursor, TryFromCursor},
    types::{
        fixnum::{FixNum, FixNumLen},
        float::RbFloat,
        regex::RbRegexStr,
        string::RbStr,
        type_byte::MarshalTypeByte,
    },
    version_number::VersionNumber,
};

pub mod config;
mod enum_access;
mod error;
mod map;
mod seq;

pub use config::DeserializerConfig;
pub(crate) use error::ErrorKind;
pub use error::MarshalDeserializeError;
use error::type_mismatch;

/// Deserialize a `T` from raw Ruby Marshal bytes.
pub fn from_bytes<'a, T>(input: &'a [u8]) -> Result<T, MarshalDeserializeError>
where
    T: serde_core::de::Deserialize<'a>,
{
    from_bytes_with_config(input, DeserializerConfig::new())
}

/// Deserialize a `T` from raw Ruby Marshal bytes with a [`DeserializerConfig`].
pub fn from_bytes_with_config<'a, T>(
    input: &'a [u8],
    config: DeserializerConfig,
) -> Result<T, MarshalDeserializeError>
where
    T: serde_core::de::Deserialize<'a>,
{
    let mut de = Deserializer::with_config(input, config)?;
    T::deserialize(&mut de)
}

/// Maximum nesting depth when replaying object references (`@`).
const MAX_REF_DEPTH: usize = 128;

/// Deserializer over Ruby marshal bytes
pub struct Deserializer<'de> {
    cursor: Cursor<'de>,
    config: DeserializerConfig,
    version: VersionNumber,
    symbols: Vec<&'de RbStr>,
    objects: Vec<NonZeroUsize>,
    /// Offset of a wrapper (`I`/`e`/`C`) waiting for its inner value to claim it in the object table.
    /// See [`Self::next_type_byte`].
    pending_wrapper: Option<NonZeroUsize>,
    replay_depth: usize,
}

impl<'de> Deserializer<'de> {
    /// Create a deserializer with the default (strict) [`DeserializerConfig`], checking the version number.
    pub fn new(input: &'de [u8]) -> Result<Self, MarshalDeserializeError> {
        Self::with_config(input, DeserializerConfig::new())
    }

    /// Create a deserializer with the given [`DeserializerConfig`], checking the version number.
    pub fn with_config(
        input: &'de [u8],
        config: DeserializerConfig,
    ) -> Result<Self, MarshalDeserializeError> {
        let mut cursor = Cursor::new(input);
        let version: VersionNumber = cursor.take().ok_or(ErrorKind::UnexpectedEof)?;
        if !version.can_read() {
            return Err(ErrorKind::VersionNumber(version).into());
        }

        Ok(Self {
            cursor,
            config,
            version,
            symbols: Vec::new(),
            objects: Vec::new(),
            pending_wrapper: None,
            replay_depth: 0,
        })
    }

    /// The version number from the start of the input.
    pub fn version(&self) -> VersionNumber {
        self.version
    }

    fn take<T: FromCursor<'de>>(&mut self) -> Result<T, MarshalDeserializeError> {
        self.cursor
            .take()
            .ok_or(ErrorKind::UnexpectedEof)
            .map_err(Into::into)
    }

    fn try_take<T>(&mut self) -> Result<T, MarshalDeserializeError>
    where
        T: TryFromCursor<'de>,
        ErrorKind: From<T::Error>,
    {
        match self.cursor.try_take::<T>() {
            None => Err(ErrorKind::UnexpectedEof.into()),
            Some(Ok(val)) => Ok(val),
            Some(Err(e)) => Err(ErrorKind::from(e).into()),
        }
    }

    fn take_n(&mut self, n: usize) -> Result<&'de [u8], MarshalDeserializeError> {
        self.cursor
            .take_n(n)
            .ok_or(ErrorKind::UnexpectedEof)
            .map_err(Into::into)
    }

    /// Read the type byte of the value starting at the cursor, maintaining the object table.
    ///
    /// Ruby assigns object table entries in pre-order: a value enters the table the moment its
    /// type byte is read, before any of its children. The wrappers `I`/`e`/`C` are the
    /// exception: they don't get an entry of their own, their inner value's entry points at
    /// the (outermost) wrapper instead so that replaying the entry reproduces the whole
    /// construct.
    fn next_type_byte(&mut self) -> Result<MarshalTypeByte, MarshalDeserializeError> {
        let offset = NonZeroUsize::new(self.cursor.pos());
        let type_byte: MarshalTypeByte = self.try_take()?;

        if self.replay_depth == 0 {
            match type_byte {
                // wrappers: the inner value claims the outermost wrapper's offset
                MarshalTypeByte::Instance
                | MarshalTypeByte::Extended
                | MarshalTypeByte::UserString => {
                    if self.pending_wrapper.is_none() {
                        self.pending_wrapper = offset;
                    }
                }
                // these never enter the object table; a wrapper offset left dangling by e.g.
                // an ivar'd symbol (`I:`) dies here, before the ivars could claim it
                MarshalTypeByte::Nil
                | MarshalTypeByte::True
                | MarshalTypeByte::False
                | MarshalTypeByte::Fixnum
                | MarshalTypeByte::Symbol
                | MarshalTypeByte::SymbolLink
                | MarshalTypeByte::ObjectReference => {
                    self.pending_wrapper = None;
                }
                // everything else registers
                _ => {
                    let offset = self
                        .pending_wrapper
                        .take()
                        .or(offset)
                        .expect("a marshal value cannot start at offset 0");
                    self.objects.push(offset);
                }
            }
        }

        Ok(type_byte)
    }

    /// Finish reading a symbol whose type byte (`:` or `;`) was already consumed.
    fn finish_symbol(
        &mut self,
        type_byte: MarshalTypeByte,
    ) -> Result<&'de RbStr, MarshalDeserializeError> {
        match type_byte {
            MarshalTypeByte::Symbol => {
                let rb: &'de RbStr = self.try_take()?;
                if self.replay_depth == 0 {
                    self.symbols.push(rb);
                }
                Ok(rb)
            }
            MarshalTypeByte::SymbolLink => {
                let link = self.take::<FixNum>()?.inner();
                usize::try_from(link)
                    .ok()
                    .and_then(|idx| self.symbols.get(idx).copied())
                    .ok_or_else(|| ErrorKind::InvalidSymbolLink(link).into())
            }
            other => Err(ErrorKind::ExpectedSymbol(other).into()),
        }
    }

    /// Parse a symbol (`:` or `;`) in a position where only a symbol is allowed (class names,
    /// ivar keys). These bypass [`Self::next_type_byte`] since they never touch the object table.
    pub(crate) fn parse_symbol(&mut self) -> Result<&'de RbStr, MarshalDeserializeError> {
        let type_byte: MarshalTypeByte = self.try_take()?;
        self.finish_symbol(type_byte)
    }

    /// [`Self::parse_symbol`] converted to UTF-8.
    fn symbol_str(&mut self) -> Result<&'de str, MarshalDeserializeError> {
        rb_str_to_str(self.parse_symbol()?)
    }

    /// Resolve an object reference (`@`, type byte already consumed) by replaying the bytes of
    /// the referenced value, continuing with `f` from there.
    fn resolve_ref<R>(
        &mut self,
        f: &mut dyn FnMut(&mut Self, MarshalTypeByte) -> Result<R, MarshalDeserializeError>,
    ) -> Result<R, MarshalDeserializeError> {
        let link = self.take::<FixNum>()?.inner();
        let offset = usize::try_from(link)
            .ok()
            .and_then(|idx| self.objects.get(idx).copied())
            .ok_or(ErrorKind::InvalidObjectRef(link))?;

        if self.replay_depth >= MAX_REF_DEPTH {
            return Err(ErrorKind::CyclicRef.into());
        }

        let return_to = self.cursor.pos();
        self.cursor.set_pos(offset.get());
        self.replay_depth += 1;
        let result = self.parse_value_dyn(f);
        self.replay_depth -= 1;
        self.cursor.set_pos(return_to);
        result
    }

    /// Drive `f` with the concrete type byte of the next value, transparently resolving object
    /// references and unwrapping whichever wrappers the [`DeserializerConfig`] flattens.
    fn parse_value<R>(
        &mut self,
        f: impl FnOnce(&mut Self, MarshalTypeByte) -> Result<R, MarshalDeserializeError>,
    ) -> Result<R, MarshalDeserializeError> {
        let mut f = Some(f);
        self.parse_value_dyn(&mut move |de, type_byte| {
            (f.take().expect("parse_value dispatched twice"))(de, type_byte)
        })
    }

    // `dyn` so the reference/wrapper recursion doesn't monomorphise infinitely
    fn parse_value_dyn<R>(
        &mut self,
        f: &mut dyn FnMut(&mut Self, MarshalTypeByte) -> Result<R, MarshalDeserializeError>,
    ) -> Result<R, MarshalDeserializeError> {
        let type_byte = self.next_type_byte()?;
        match type_byte {
            MarshalTypeByte::ObjectReference => self.resolve_ref(f),
            MarshalTypeByte::Instance if self.config.ivar_as_inner => {
                let value = self.parse_value_dyn(f)?;
                self.skip_ivars()?;
                Ok(value)
            }
            MarshalTypeByte::Extended
            | MarshalTypeByte::UserString
            | MarshalTypeByte::UserMarshal
            | MarshalTypeByte::Data
                if self.config.classed_as_inner =>
            {
                self.parse_symbol()?;
                self.parse_value_dyn(f)
            }
            concrete => f(self, concrete),
        }
    }

    /// Parse past the value at the cursor without materialising it. Table bookkeeping still
    /// happens so that later symbol/object links stay aligned.
    pub(crate) fn skip_value(&mut self) -> Result<(), MarshalDeserializeError> {
        let type_byte = self.next_type_byte()?;
        match type_byte {
            MarshalTypeByte::Nil | MarshalTypeByte::True | MarshalTypeByte::False => Ok(()),
            MarshalTypeByte::Fixnum | MarshalTypeByte::ObjectReference => {
                self.take::<FixNum>().map(drop)
            }
            MarshalTypeByte::Symbol | MarshalTypeByte::SymbolLink => {
                self.finish_symbol(type_byte).map(drop)
            }
            MarshalTypeByte::Float
            | MarshalTypeByte::String
            | MarshalTypeByte::Class
            | MarshalTypeByte::Module
            | MarshalTypeByte::ClassOrModule => self.try_take::<&RbStr>().map(drop),
            MarshalTypeByte::Bignum => {
                self.take::<u8>()?; // sign
                let len: FixNumLen = self.try_take()?;
                self.take_n(len.inner() * 2).map(drop)
            }
            MarshalTypeByte::RegularExpression => self.try_take::<RbRegexStr>().map(drop),
            MarshalTypeByte::Array => {
                let len: FixNumLen = self.try_take()?;
                for _ in 0..len.inner() {
                    self.skip_value()?;
                }
                Ok(())
            }
            MarshalTypeByte::Hash => {
                let len: FixNumLen = self.try_take()?;
                self.skip_hash_pairs(len.inner())
            }
            MarshalTypeByte::HashDefault => {
                let len: FixNumLen = self.try_take()?;
                self.skip_hash_pairs(len.inner())?;
                self.skip_value() // the default
            }
            MarshalTypeByte::Instance => {
                self.skip_value()?;
                self.skip_ivars()
            }
            MarshalTypeByte::Object | MarshalTypeByte::Struct => {
                self.parse_symbol()?;
                self.skip_ivars()
            }
            MarshalTypeByte::Extended
            | MarshalTypeByte::UserString
            | MarshalTypeByte::UserMarshal
            | MarshalTypeByte::Data => {
                self.parse_symbol()?;
                self.skip_value()
            }
            MarshalTypeByte::UserDefined => {
                self.parse_symbol()?;
                let len: FixNumLen = self.try_take()?;
                self.take_n(len.inner()).map(drop)
            }
        }
    }

    /// Skip an ivar/member list: a count followed by symbol-value pairs.
    pub(crate) fn skip_ivars(&mut self) -> Result<(), MarshalDeserializeError> {
        let len: FixNumLen = self.try_take()?;
        for _ in 0..len.inner() {
            self.parse_symbol()?;
            self.skip_value()?;
        }
        Ok(())
    }

    /// Skip `pairs` key-value pairs.
    pub(crate) fn skip_hash_pairs(&mut self, pairs: usize) -> Result<(), MarshalDeserializeError> {
        for _ in 0..pairs {
            self.skip_value()?;
            self.skip_value()?;
        }
        Ok(())
    }

    /// visit_seq a Ruby Array of `len` elements, parsing past anything the visitor leaves.
    fn drive_seq<V: Visitor<'de>>(
        &mut self,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        let mut access = ArraySeqAccess::new(self, len);
        let value = visitor.visit_seq(&mut access)?;
        access.finish()?;
        Ok(value)
    }

    /// visit_map `pairs` key-value pairs, parsing past anything the visitor leaves.
    pub(crate) fn drive_hash<V: Visitor<'de>>(
        &mut self,
        pairs: usize,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        let mut access = HashMapAccess::new(self, pairs);
        let value = visitor.visit_map(&mut access)?;
        access.finish()?;
        Ok(value)
    }

    /// visit_map an ivar/member list (count followed by symbol-value pairs), parsing past
    /// anything the visitor leaves.
    pub(crate) fn drive_ivar_map<V: Visitor<'de>>(
        &mut self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        let len: FixNumLen = self.try_take()?;
        let mut access = IvarMapAccess::new(self, len.inner());
        let value = visitor.visit_map(&mut access)?;
        access.finish()?;
        Ok(value)
    }
}

pub(crate) fn rb_str_to_str(rb: &RbStr) -> Result<&str, MarshalDeserializeError> {
    rb.try_into()
        .map_err(ErrorKind::InvalidUtf8)
        .map_err(MarshalDeserializeError::from)
}

macro_rules! deserialize_int {
    ($self:ident, $visitor:ident, $visit:ident, $ty:ty) => {
        $self.parse_value(|de, type_byte| match type_byte {
            MarshalTypeByte::Fixnum => {
                let n = de.take::<FixNum>()?.inner();
                let v: $ty = n.try_into().map_err(|_| ErrorKind::IntegerOverflowI32 {
                    target_type: stringify!($ty),
                    value: n,
                })?;
                $visitor.$visit(v)
            }
            MarshalTypeByte::Bignum => {
                let big: BigInt = de.try_take()?;
                let v: $ty = (&big)
                    .try_into()
                    .map_err(|_| ErrorKind::IntegerOverflowBigInt {
                        target_type: stringify!($ty),
                        value: big.clone(),
                    })?;
                $visitor.$visit(v)
            }
            other => Err(type_mismatch("integer", other)),
        })
    };
}

impl<'de> serde_core::de::Deserializer<'de> for &mut Deserializer<'de> {
    type Error = MarshalDeserializeError;

    fn deserialize_any<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.parse_value(|de, type_byte| match type_byte {
            MarshalTypeByte::Nil => visitor.visit_unit(),
            MarshalTypeByte::True => visitor.visit_bool(true),
            MarshalTypeByte::False => visitor.visit_bool(false),
            MarshalTypeByte::Fixnum => visitor.visit_i32(de.take::<FixNum>()?.inner()),
            MarshalTypeByte::Float => visitor.visit_f64(de.try_take::<RbFloat>()?.inner()),
            MarshalTypeByte::Bignum => {
                let big: BigInt = de.try_take()?;
                if let Ok(v) = i64::try_from(&big) {
                    visitor.visit_i64(v)
                } else if let Ok(v) = u64::try_from(&big) {
                    visitor.visit_u64(v)
                } else if let Ok(v) = i128::try_from(&big) {
                    visitor.visit_i128(v)
                } else if let Ok(v) = u128::try_from(&big) {
                    visitor.visit_u128(v)
                } else {
                    Err(ErrorKind::BignumTooLarge.into())
                }
            }
            MarshalTypeByte::Symbol | MarshalTypeByte::SymbolLink => {
                let rb = de.finish_symbol(type_byte)?;
                match <&str>::try_from(rb) {
                    Ok(s) => visitor.visit_borrowed_str(s),
                    Err(_) => visitor.visit_borrowed_bytes(rb.as_slice()),
                }
            }
            MarshalTypeByte::String
            | MarshalTypeByte::Class
            | MarshalTypeByte::Module
            | MarshalTypeByte::ClassOrModule => {
                let rb: &RbStr = de.try_take()?;
                match <&str>::try_from(rb) {
                    Ok(s) => visitor.visit_borrowed_str(s),
                    Err(_) => visitor.visit_borrowed_bytes(rb.as_slice()),
                }
            }
            MarshalTypeByte::RegularExpression => {
                let (flags, pattern) = de.try_take::<RbRegexStr>()?.inner();
                visitor.visit_seq(RegexSeqAccess::new(pattern, flags))
            }
            MarshalTypeByte::Array => {
                let len: FixNumLen = de.try_take()?;
                de.drive_seq(len.inner(), visitor)
            }
            MarshalTypeByte::Hash => {
                let len: FixNumLen = de.try_take()?;
                de.drive_hash(len.inner(), visitor)
            }
            MarshalTypeByte::HashDefault => {
                let len: FixNumLen = de.try_take()?;
                if de.config.hash_default_as_map {
                    let value = de.drive_hash(len.inner(), visitor)?;
                    de.skip_value()?; // the default
                    Ok(value)
                } else {
                    let mut access = HashDefaultSeqAccess::new(de, len.inner());
                    let value = visitor.visit_seq(&mut access)?;
                    access.finish()?;
                    Ok(value)
                }
            }
            MarshalTypeByte::Object | MarshalTypeByte::Struct => {
                if de.config.object_as_map {
                    de.parse_symbol()?;
                    de.drive_ivar_map(visitor)
                } else {
                    let name = de.symbol_str()?;
                    let mut access = ObjectSeqAccess::new(de, name);
                    let value = visitor.visit_seq(&mut access)?;
                    access.finish()?;
                    Ok(value)
                }
            }
            // `ivar_as_inner` was handled by parse_value
            MarshalTypeByte::Instance => {
                let mut access = InstanceSeqAccess::new(de);
                let value = visitor.visit_seq(&mut access)?;
                access.finish()?;
                Ok(value)
            }
            // as was `classed_as_inner`
            MarshalTypeByte::Extended
            | MarshalTypeByte::UserString
            | MarshalTypeByte::UserMarshal
            | MarshalTypeByte::Data => {
                let name = de.symbol_str()?;
                let mut access = ClassedSeqAccess::new(de, name);
                let value = visitor.visit_seq(&mut access)?;
                access.finish()?;
                Ok(value)
            }
            MarshalTypeByte::UserDefined => {
                let name = de.symbol_str()?;
                let len: FixNumLen = de.try_take()?;
                let data = de.take_n(len.inner())?;
                visitor.visit_seq(UserDefinedSeqAccess::new(name, data))
            }
            MarshalTypeByte::ObjectReference => {
                unreachable!("object references are resolved before dispatch")
            }
        })
    }

    fn deserialize_bool<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.parse_value(|_, type_byte| match type_byte {
            MarshalTypeByte::True => visitor.visit_bool(true),
            MarshalTypeByte::False => visitor.visit_bool(false),
            other => Err(type_mismatch("bool", other)),
        })
    }

    fn deserialize_i8<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        deserialize_int!(self, visitor, visit_i8, i8)
    }

    fn deserialize_i16<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        deserialize_int!(self, visitor, visit_i16, i16)
    }

    fn deserialize_i32<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        deserialize_int!(self, visitor, visit_i32, i32)
    }

    fn deserialize_i64<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        deserialize_int!(self, visitor, visit_i64, i64)
    }

    fn deserialize_i128<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        deserialize_int!(self, visitor, visit_i128, i128)
    }

    fn deserialize_u8<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        deserialize_int!(self, visitor, visit_u8, u8)
    }

    fn deserialize_u16<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        deserialize_int!(self, visitor, visit_u16, u16)
    }

    fn deserialize_u32<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        deserialize_int!(self, visitor, visit_u32, u32)
    }

    fn deserialize_u64<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        deserialize_int!(self, visitor, visit_u64, u64)
    }

    fn deserialize_u128<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        deserialize_int!(self, visitor, visit_u128, u128)
    }

    fn deserialize_f32<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.parse_value(|de, type_byte| match type_byte {
            MarshalTypeByte::Float => visitor.visit_f32(de.try_take::<RbFloat>()?.inner() as f32),
            MarshalTypeByte::Fixnum => visitor.visit_f32(de.take::<FixNum>()?.inner() as f32),
            other => Err(type_mismatch("float", other)),
        })
    }

    fn deserialize_f64<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.parse_value(|de, type_byte| match type_byte {
            MarshalTypeByte::Float => visitor.visit_f64(de.try_take::<RbFloat>()?.inner()),
            MarshalTypeByte::Fixnum => visitor.visit_f64(de.take::<FixNum>()?.inner() as f64),
            other => Err(type_mismatch("float", other)),
        })
    }

    fn deserialize_char<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.parse_value(|de, type_byte| {
            let rb = match type_byte {
                MarshalTypeByte::String => de.try_take::<&RbStr>()?,
                MarshalTypeByte::Symbol | MarshalTypeByte::SymbolLink => {
                    de.finish_symbol(type_byte)?
                }
                other => return Err(type_mismatch("char", other)),
            };
            let s = rb_str_to_str(rb)?;
            let mut chars = s.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => visitor.visit_char(c),
                _ => Err(ErrorKind::ExpectedSingleChar {
                    len: s.chars().count(),
                }
                .into()),
            }
        })
    }

    fn deserialize_str<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.parse_value(|de, type_byte| {
            let rb = match type_byte {
                MarshalTypeByte::String
                | MarshalTypeByte::Class
                | MarshalTypeByte::Module
                | MarshalTypeByte::ClassOrModule => de.try_take::<&RbStr>()?,
                MarshalTypeByte::Symbol | MarshalTypeByte::SymbolLink => {
                    de.finish_symbol(type_byte)?
                }
                MarshalTypeByte::RegularExpression => de.try_take::<RbRegexStr>()?.inner().1,
                other => return Err(type_mismatch("string", other)),
            };
            visitor.visit_borrowed_str(rb_str_to_str(rb)?)
        })
    }

    fn deserialize_string<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.deserialize_str(visitor)
    }

    fn deserialize_bytes<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.parse_value(|de, type_byte| match type_byte {
            MarshalTypeByte::String => {
                visitor.visit_borrowed_bytes(de.try_take::<&RbStr>()?.as_slice())
            }
            MarshalTypeByte::Symbol | MarshalTypeByte::SymbolLink => {
                visitor.visit_borrowed_bytes(de.finish_symbol(type_byte)?.as_slice())
            }
            MarshalTypeByte::UserDefined => {
                de.parse_symbol()?;
                let len: FixNumLen = de.try_take()?;
                visitor.visit_borrowed_bytes(de.take_n(len.inner())?)
            }
            other => Err(type_mismatch("bytes", other)),
        })
    }

    fn deserialize_byte_buf<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.deserialize_bytes(visitor)
    }

    fn deserialize_option<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        // nil is never the target of an object reference (Ruby doesn't register it), so
        // peeking the raw byte is enough
        match self.cursor.peek() {
            Some(b'0') => {
                self.next_type_byte()?;
                visitor.visit_none()
            }
            Some(_) => visitor.visit_some(self),
            None => Err(ErrorKind::UnexpectedEof.into()),
        }
    }

    fn deserialize_unit<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.parse_value(|_, type_byte| match type_byte {
            MarshalTypeByte::Nil => visitor.visit_unit(),
            other => Err(type_mismatch("nil/unit", other)),
        })
    }

    fn deserialize_unit_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.deserialize_unit(visitor)
    }

    fn deserialize_newtype_struct<V: Visitor<'de>>(
        self,
        _name: &'static str,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        visitor.visit_newtype_struct(self)
    }

    fn deserialize_seq<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.parse_value(|de, type_byte| match type_byte {
            MarshalTypeByte::Array => {
                let len: FixNumLen = de.try_take()?;
                de.drive_seq(len.inner(), visitor)
            }
            MarshalTypeByte::Object | MarshalTypeByte::Struct if !de.config.object_as_map => {
                let name = de.symbol_str()?;
                let mut access = ObjectSeqAccess::new(de, name);
                let value = visitor.visit_seq(&mut access)?;
                access.finish()?;
                Ok(value)
            }
            MarshalTypeByte::Instance => {
                let mut access = InstanceSeqAccess::new(de);
                let value = visitor.visit_seq(&mut access)?;
                access.finish()?;
                Ok(value)
            }
            MarshalTypeByte::Extended
            | MarshalTypeByte::UserString
            | MarshalTypeByte::UserMarshal
            | MarshalTypeByte::Data => {
                let name = de.symbol_str()?;
                let mut access = ClassedSeqAccess::new(de, name);
                let value = visitor.visit_seq(&mut access)?;
                access.finish()?;
                Ok(value)
            }
            MarshalTypeByte::UserDefined => {
                let name = de.symbol_str()?;
                let len: FixNumLen = de.try_take()?;
                let data = de.take_n(len.inner())?;
                visitor.visit_seq(UserDefinedSeqAccess::new(name, data))
            }
            MarshalTypeByte::RegularExpression => {
                let (flags, pattern) = de.try_take::<RbRegexStr>()?.inner();
                visitor.visit_seq(RegexSeqAccess::new(pattern, flags))
            }
            MarshalTypeByte::HashDefault if !de.config.hash_default_as_map => {
                let len: FixNumLen = de.try_take()?;
                let mut access = HashDefaultSeqAccess::new(de, len.inner());
                let value = visitor.visit_seq(&mut access)?;
                access.finish()?;
                Ok(value)
            }
            other => Err(type_mismatch("sequence", other)),
        })
    }

    fn deserialize_tuple<V: Visitor<'de>>(
        self,
        _: usize,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_tuple_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: usize,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.deserialize_seq(visitor)
    }

    fn deserialize_map<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.parse_value(|de, type_byte| match type_byte {
            MarshalTypeByte::Hash => {
                let len: FixNumLen = de.try_take()?;
                de.drive_hash(len.inner(), visitor)
            }
            MarshalTypeByte::HashDefault => {
                let len: FixNumLen = de.try_take()?;
                let value = de.drive_hash(len.inner(), visitor)?;
                de.skip_value()?; // the default
                Ok(value)
            }
            MarshalTypeByte::Object | MarshalTypeByte::Struct if de.config.object_as_map => {
                de.parse_symbol()?;
                de.drive_ivar_map(visitor)
            }
            other => Err(type_mismatch("map", other)),
        })
    }

    fn deserialize_struct<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.deserialize_map(visitor)
    }

    fn deserialize_enum<V: Visitor<'de>>(
        self,
        _: &'static str,
        _: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.parse_value(|de, type_byte| match type_byte {
            MarshalTypeByte::Symbol | MarshalTypeByte::SymbolLink => {
                let name = rb_str_to_str(de.finish_symbol(type_byte)?)?;
                visitor.visit_enum(UnitVariantDeserializer { name })
            }
            MarshalTypeByte::String => {
                let name = rb_str_to_str(de.try_take::<&RbStr>()?)?;
                visitor.visit_enum(UnitVariantDeserializer { name })
            }
            MarshalTypeByte::Hash => {
                let len: FixNumLen = de.try_take()?;
                if len.inner() != 1 {
                    return Err(ErrorKind::EnumHashLen(len.inner()).into());
                }
                visitor.visit_enum(MapVariantDeserializer { de })
            }
            other => Err(type_mismatch("enum (symbol or single-entry hash)", other)),
        })
    }

    fn deserialize_identifier<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.deserialize_str(visitor)
    }

    fn deserialize_ignored_any<V: Visitor<'de>>(
        self,
        visitor: V,
    ) -> Result<V::Value, MarshalDeserializeError> {
        self.skip_value()?;
        visitor.visit_unit()
    }
}

// vast majority of tests are AI generated, I probably wouldn't do it otherwise.
#[cfg(test)]
mod tests {

    use std::collections::HashMap;

    use serde::Deserialize;

    use crate::{
        deserializer::{
            DeserializerConfig, MarshalDeserializeError, from_bytes, from_bytes_with_config,
        },
        deserializer_types::{
            Ignored,
            ivar::{Ivar, WithEncoding},
            rb_object::RbObject,
            transparent::Transparent,
        },
        types::encoding::RubyEncoding,
    };

    // Helper to deserialize from raw marshal bytes
    fn de_from_ruby<'a, T: Deserialize<'a>>(bytes: &'a [u8]) -> Result<T, MarshalDeserializeError> {
        from_bytes(bytes)
    }

    #[test]
    fn test_nil_to_unit() {
        // Marshal.dump(nil) = \x04\x080
        let bytes = b"\x04\x080";
        let result: () = de_from_ruby(bytes).unwrap();
        assert_eq!(result, ());
    }

    #[test]
    fn test_nil_to_option() {
        let bytes = b"\x04\x080";
        let result: Option<i32> = de_from_ruby(bytes).unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_bool_true() {
        // Marshal.dump(true) = \x04\x08T
        let bytes = b"\x04\x08T";
        let result: bool = de_from_ruby(bytes).unwrap();
        assert!(result);
    }

    #[test]
    fn test_bool_false() {
        let bytes = b"\x04\x08F";
        let result: bool = de_from_ruby(bytes).unwrap();
        assert!(!result);
    }

    #[test]
    fn test_fixnum_zero() {
        // Marshal.dump(0) = \x04\x08i\x00
        let bytes = b"\x04\x08i\x00";
        let result: i32 = de_from_ruby(bytes).unwrap();
        assert_eq!(result, 0);
    }

    #[test]
    fn test_fixnum_positive() {
        // Marshal.dump(42) = \x04\x08i/
        let bytes = b"\x04\x08i/";
        let result: i32 = de_from_ruby(bytes).unwrap();
        assert_eq!(result, 42);
    }

    #[test]
    fn test_fixnum_to_option_some() {
        let bytes = b"\x04\x08i/";
        let result: Option<i32> = de_from_ruby(bytes).unwrap();
        assert_eq!(result, Some(42));
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_float() {
        // Marshal.dump(3.14) = \x04\x08f\x093.14
        let bytes = b"\x04\x08f\x093.14";
        let result: f64 = de_from_ruby(bytes).unwrap();
        assert!((result - 3.14).abs() < f64::EPSILON);
    }

    #[test]
    fn test_array_of_ints() {
        // Marshal.dump([1,2,3]) = \x04\x08[\x08i\x06i\x07i\x08
        let bytes = b"\x04\x08[\x08i\x06i\x07i\x08";
        let result: Vec<i32> = de_from_ruby(bytes).unwrap();
        assert_eq!(result, vec![1, 2, 3]);
    }

    #[test]
    fn test_symbol() {
        // Marshal.dump(:hello) = \x04\x08:\x0ahello
        let bytes = b"\x04\x08:\x0ahello";
        let result: &str = de_from_ruby(bytes).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_string_with_instance_wrapper() {
        // Marshal.dump("hello") = \x04\x08I\"\x0ahello\x06:\x06ET
        // Instance wrapping is explicit by default - use Ivar<T> to unwrap
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let result: Ivar<&str> = de_from_ruby(bytes).unwrap();
        assert_eq!(result.inner, "hello");
    }

    #[test]
    fn test_string_ivar_with_ivars_captured() {
        // Same data, but capture the encoding ivar
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let result: Ivar<&str, HashMap<&str, bool>> = de_from_ruby(bytes).unwrap();
        assert_eq!(result.inner, "hello");
        assert_eq!(result.ivars.get("E"), Some(&true));
    }

    #[test]
    fn test_instance_as_tuple() {
        // Instance can also be deserialized as a raw tuple
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let result: (&str, HashMap<&str, bool>) = de_from_ruby(bytes).unwrap();
        assert_eq!(result.0, "hello");
        assert_eq!(result.1.get("E"), Some(&true));
    }

    #[test]
    fn test_hash_to_hashmap() {
        // Marshal.dump({a: 1, b: 2}) = hash with symbol keys
        // \x04\x08{\x07:\x06ai\x06:\x06bi\x07
        let bytes = b"\x04\x08{\x07:\x06ai\x06:\x06bi\x07";
        let result: HashMap<&str, i32> = de_from_ruby(bytes).unwrap();
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
        let result: Point = de_from_ruby(bytes).unwrap();
        assert_eq!(result, Point { x: 10, y: 20 });
    }

    #[test]
    fn test_nested_array() {
        // Marshal.dump([1, [2, 3]])
        let bytes = b"\x04\x08[\x07i\x06[\x07i\x07i\x08";
        let result: (i32, Vec<i32>) = de_from_ruby(bytes).unwrap();
        assert_eq!(result, (1, vec![2, 3]));
    }

    // ---- Ivar deserialization tests ----

    #[test]
    fn ivar_discard_ivars() {
        // Marshal.dump("hello") = I"\x0ahello\x06:\x06ET
        // Ivar<T> (O=Ignored) discards the encoding ivar
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let result: Ivar<&str> = de_from_ruby(bytes).unwrap();
        assert_eq!(result.inner, "hello");
        assert_eq!(result.ivars, Ignored);
    }

    #[test]
    fn ivar_capture_as_hashmap() {
        // Capture ivars into a HashMap
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let result: Ivar<&str, HashMap<&str, bool>> = de_from_ruby(bytes).unwrap();
        assert_eq!(result.inner, "hello");
        assert_eq!(result.ivars.len(), 1);
        assert!(result.ivars["E"]);
    }

    #[test]
    fn ivar_as_raw_tuple() {
        // Instance can be deserialized as a plain tuple
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let result: (&str, HashMap<&str, bool>) = de_from_ruby(bytes).unwrap();
        assert_eq!(result.0, "hello");
        assert!(result.1["E"]);
    }

    #[test]
    fn ivar_deref_to_inner() {
        // Ivar<T> derefs to T
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let result: Ivar<&str> = de_from_ruby(bytes).unwrap();
        let s: &str = &result;
        assert_eq!(s, "hello");
    }

    #[test]
    fn ivar_utf8_encoding() {
        // E: true → UTF-8
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let result: WithEncoding<&str> = de_from_ruby(bytes).unwrap();
        assert_eq!(result.inner, "hello");
        assert_eq!(*result.ivars, RubyEncoding::Utf8);
    }

    #[test]
    fn ivar_ascii_encoding() {
        // E: false → US-ASCII
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06EF";
        let result: WithEncoding<&str> = de_from_ruby(bytes).unwrap();
        assert_eq!(result.inner, "hello");
        assert_eq!(*result.ivars, RubyEncoding::UsAscii);
    }

    #[test]
    fn ivar_explicit_encoding() {
        // encoding: "Shift_JIS" — uses the :encoding ivar instead of :E
        // Marshal.dump("hello".encode("Shift_JIS"))
        // I"\x0ahello\x06:\x0dencoding"\x0eShift_JIS
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x0dencoding\"\x0eShift_JIS";
        let result: WithEncoding<&str> = de_from_ruby(bytes).unwrap();
        assert_eq!(result.inner, "hello");
        assert_eq!(*result.ivars, RubyEncoding::ShiftJis);
    }

    #[test]
    fn ivar_encoding_deref() {
        // Encoding derefs to RubyEncoding
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let result: WithEncoding<&str> = de_from_ruby(bytes).unwrap();
        let enc: &RubyEncoding = &result.ivars;
        assert_eq!(*enc, RubyEncoding::Utf8);
    }

    #[test]
    fn ivar_multiple_ivars() {
        // Instance with 2 ivars: E: true and another custom one
        // I"\x0ahello\x07:\x06ET:\x06xi\x2a  (E => true, x => 37)
        let bytes = b"\x04\x08I\"\x0ahello\x07:\x06ET:\x06xi\x2a";
        // Capture all ivars as a struct
        #[derive(Debug, Deserialize)]
        struct Meta {
            #[serde(rename = "E")]
            encoding: bool,
            x: i32,
        }
        let result: Ivar<&str, Meta> = de_from_ruby(bytes).unwrap();
        assert_eq!(result.inner, "hello");
        assert!(result.ivars.encoding);
        assert_eq!(result.ivars.x, 37);
    }

    #[test]
    fn ivar_encoding_ignores_extra_ivars() {
        // Encoding skips unknown ivars
        // I"\x0ahello\x07:\x06ET:\x06xi\x2a
        let bytes = b"\x04\x08I\"\x0ahello\x07:\x06ET:\x06xi\x2a";
        let result: WithEncoding<&str> = de_from_ruby(bytes).unwrap();
        assert_eq!(result.inner, "hello");
        assert_eq!(*result.ivars, RubyEncoding::Utf8);
    }

    #[test]
    fn ivar_in_array() {
        // Array of Instance-wrapped strings: ["hello", "world"]
        // [\x07 I"\x0ahello\x06:\x06ET I"\x0aworld\x06:\x06ET
        let bytes = b"\x04\x08[\x07I\"\x0ahello\x06:\x06ETI\"\x0aworld\x06:\x06ET";
        let result: Vec<Ivar<&str>> = de_from_ruby(bytes).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].inner, "hello");
        assert_eq!(result[1].inner, "world");
    }

    #[test]
    fn ivar_string_not_transparent() {
        // Deserializing an Instance-wrapped string directly as &str should fail by default
        // because Instance is a sequence, not a string
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let result: Result<&str, MarshalDeserializeError> = de_from_ruby(bytes);
        assert!(result.is_err());
    }

    #[test]
    fn ivar_with_encoding_in_array() {
        // Array of WithEncoding strings
        let bytes = b"\x04\x08[\x07I\"\x0ahello\x06:\x06ETI\"\x0aworld\x06:\x06EF";
        let result: Vec<WithEncoding<&str>> = de_from_ruby(bytes).unwrap();
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
        // RbObject<T> (N=Ignored) discards the class name
        #[derive(Debug, Deserialize, PartialEq)]
        struct Pt {
            #[serde(rename = "@x")]
            x: i32,
            #[serde(rename = "@y")]
            y: i32,
        }
        let result: RbObject<Pt> = de_from_ruby(OBJECT_PT).unwrap();
        assert_eq!(result.fields, Pt { x: 10, y: 20 });
        assert_eq!(result.class, Ignored);
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
        let result: RbObject<Pt, &str> = de_from_ruby(OBJECT_PT).unwrap();
        assert_eq!(result.class, "Pt");
        assert_eq!(result.fields, Pt { x: 10, y: 20 });
    }

    #[test]
    fn object_as_raw_tuple() {
        // Object can be deserialized as a plain tuple, useful fields first
        let result: (HashMap<&str, i32>, &str) = de_from_ruby(OBJECT_PT).unwrap();
        assert_eq!(result.0["@x"], 10);
        assert_eq!(result.0["@y"], 20);
        assert_eq!(result.1, "Pt");
    }

    #[test]
    fn object_deref_to_fields() {
        // RbObject<T> derefs to T
        #[derive(Debug, Deserialize)]
        struct Pt {
            #[serde(rename = "@x")]
            x: i32,
        }
        let result: RbObject<Pt> = de_from_ruby(OBJECT_PT).unwrap();
        assert_eq!(result.x, 10); // accessed through Deref
    }

    #[test]
    fn object_not_transparent() {
        // Deserializing an Object directly as a struct (without RbObject) should fail by
        // default because Object is a sequence, not a map
        #[derive(Debug, Deserialize)]
        #[allow(dead_code)]
        struct Pt {
            #[serde(rename = "@x")]
            x: i32,
        }
        let result: Result<Pt, MarshalDeserializeError> = de_from_ruby(OBJECT_PT);
        assert!(result.is_err());
    }

    #[test]
    fn object_fields_as_hashmap() {
        // Fields can be captured as a HashMap
        let result: RbObject<HashMap<&str, i32>> = de_from_ruby(OBJECT_PT).unwrap();
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
        let result: RbObject<Pt> = de_from_ruby(STRUCT_PT).unwrap();
        assert_eq!(result.fields, Pt { x: 10, y: 20 });
    }

    #[test]
    fn struct_capture_name() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Pt {
            x: i32,
            y: i32,
        }
        let result: RbObject<Pt, &str> = de_from_ruby(STRUCT_PT).unwrap();
        assert_eq!(result.class, "Pt");
        assert_eq!(result.fields, Pt { x: 10, y: 20 });
    }

    #[test]
    fn struct_as_raw_tuple() {
        let result: (HashMap<&str, i32>, &str) = de_from_ruby(STRUCT_PT).unwrap();
        assert_eq!(result.0["x"], 10);
        assert_eq!(result.0["y"], 20);
        assert_eq!(result.1, "Pt");
    }

    #[test]
    fn object_in_array() {
        // Array of two Pt Objects: {x:1, y:2} and {x:3, y:4}
        let bytes = b"\x04\x08[\x07o:\x07Pt\x07:\x07@xi\x06:\x07@yi\x07o:\x07Pt\x07:\x07@xi\x08:\x07@yi\x09";
        #[derive(Debug, Deserialize, PartialEq)]
        struct Pt {
            #[serde(rename = "@x")]
            x: i32,
            #[serde(rename = "@y")]
            y: i32,
        }
        let result: Vec<RbObject<Pt>> = de_from_ruby(bytes).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].fields, Pt { x: 1, y: 2 });
        assert_eq!(result[1].fields, Pt { x: 3, y: 4 });
    }

    // ---- Object reference (`@`) tests ----

    // Marshal.dump([a, a]) where a = "hello":
    // the array registers as object 0, the ivar'd string as object 1, so the second
    // element is a link to 1: [\x07 I"\x0ahello\x06:\x06ET @\x06
    const SHARED_STRING: &[u8] = b"\x04\x08[\x07I\"\x0ahello\x06:\x06ET@\x06";

    #[test]
    fn object_ref_resolves_to_shared_string() {
        let result: Vec<Ivar<&str>> = de_from_ruby(SHARED_STRING).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].inner, "hello");
        assert_eq!(result[1].inner, "hello");
    }

    #[test]
    fn object_ref_restores_position() {
        // Marshal.dump([a, a, 42]) — parsing must continue after the link correctly
        let bytes = b"\x04\x08[\x08I\"\x0ahello\x06:\x06ET@\x06i\x2f";
        let result: (Ivar<&str>, Ivar<&str>, i32) = de_from_ruby(bytes).unwrap();
        assert_eq!(result.0.inner, "hello");
        assert_eq!(result.1.inner, "hello");
        assert_eq!(result.2, 42);
    }

    #[test]
    fn object_ref_resolves_after_skipped_value() {
        // The first element is ignored, but the link to it must still resolve:
        // skipping a value keeps registering objects
        let result: (Ignored, Ivar<&str>) = de_from_ruby(SHARED_STRING).unwrap();
        assert_eq!(result.1.inner, "hello");
    }

    #[test]
    fn self_referential_array_errors_instead_of_hanging() {
        // a = []; a << a; Marshal.dump(a) = [\x06@\x00 — the array links to itself
        #[derive(Debug, Deserialize)]
        struct Recursive(#[allow(dead_code)] Vec<Recursive>);

        let bytes = b"\x04\x08[\x06@\x00";
        let result: Result<Recursive, MarshalDeserializeError> = de_from_ruby(bytes);
        assert!(result.is_err());
    }

    #[test]
    fn invalid_object_ref_errors() {
        // a link to an object that was never registered
        let bytes = b"\x04\x08[\x06@\x0a";
        let result: Result<Vec<Ivar<&str>>, MarshalDeserializeError> = de_from_ruby(bytes);
        assert!(result.is_err());
    }

    // ---- DeserializerConfig tests ----

    #[test]
    fn config_ivar_as_inner() {
        // With ivar_as_inner the Instance wrapper vanishes
        let config = DeserializerConfig::new().with_ivar_as_inner(true);
        let bytes = b"\x04\x08I\"\x0ahello\x06:\x06ET";
        let result: &str = from_bytes_with_config(bytes, config).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn config_ivar_as_inner_in_array() {
        let config = DeserializerConfig::new().with_ivar_as_inner(true);
        let bytes = b"\x04\x08[\x07I\"\x0ahello\x06:\x06ETI\"\x0aworld\x06:\x06ET";
        let result: Vec<&str> = from_bytes_with_config(bytes, config).unwrap();
        assert_eq!(result, vec!["hello", "world"]);
    }

    #[test]
    fn config_object_as_map() {
        // With object_as_map an Object deserializes straight into a struct
        #[derive(Debug, Deserialize, PartialEq)]
        struct Pt {
            #[serde(rename = "@x")]
            x: i32,
            #[serde(rename = "@y")]
            y: i32,
        }
        let config = DeserializerConfig::new().with_object_as_map(true);
        let result: Pt = from_bytes_with_config(OBJECT_PT, config).unwrap();
        assert_eq!(result, Pt { x: 10, y: 20 });
    }

    #[test]
    fn config_object_as_map_applies_to_structs() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Pt {
            x: i32,
            y: i32,
        }
        let config = DeserializerConfig::new().with_object_as_map(true);
        let result: Pt = from_bytes_with_config(STRUCT_PT, config).unwrap();
        assert_eq!(result, Pt { x: 10, y: 20 });
    }

    #[test]
    fn config_classed_as_inner() {
        // Marshal.dump of a UserMarshal class whose marshal_dump returns [1]
        // U:\x08Foo[\x06i\x06
        let config = DeserializerConfig::new().with_classed_as_inner(true);
        let bytes = b"\x04\x08U:\x08Foo[\x06i\x06";
        let result: Vec<i32> = from_bytes_with_config(bytes, config).unwrap();
        assert_eq!(result, vec![1]);
    }

    #[test]
    fn config_hash_default_as_map() {
        // Marshal.dump(Hash.new(42).tap { |h| h[:a] = 1 }) = }\x06:\x06ai\x06i\x2f
        let config = DeserializerConfig::new().with_hash_default_as_map(true);
        let bytes = b"\x04\x08}\x06:\x06ai\x06i\x2f";
        let result: HashMap<&str, i32> = from_bytes_with_config(bytes, config).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result["a"], 1);
    }

    #[test]
    fn config_opinionated_combo() {
        // An Object with an ivar'd string field deserializes straight into plain Rust types
        // o:\x07Pt\x06:\x0a@nameI"\x0ahello\x06:\x06ET
        #[derive(Debug, Deserialize, PartialEq)]
        struct Pt<'a> {
            #[serde(rename = "@name")]
            name: &'a str,
        }
        let bytes = b"\x04\x08o:\x07Pt\x06:\x0a@nameI\"\x0ahello\x06:\x06ET";
        let result: Pt = from_bytes_with_config(bytes, DeserializerConfig::opinionated()).unwrap();
        assert_eq!(result, Pt { name: "hello" });
    }

    #[test]
    fn hash_default_as_plain_map_without_config() {
        // deserialize_map has always accepted a HashDefault, skipping the default
        let bytes = b"\x04\x08}\x06:\x06ai\x06i\x2f";
        let result: HashMap<&str, i32> = de_from_ruby(bytes).unwrap();
        assert_eq!(result["a"], 1);
    }

    #[test]
    fn transparent_resolves_refs() {
        let result: Vec<Transparent<&str>> = de_from_ruby(SHARED_STRING).unwrap();
        assert_eq!(*result[0], "hello");
        assert_eq!(*result[1], "hello");
    }

    // ---- Deeply wrapped values ----

    // Marshal.dump(MyString.new("hello")) where: class MyString < String; end
    // an Instance wrapping a UserClass wrapping a String: I C:\x0dMyString "\x0ahello ivars
    const SUBCLASSED_STRING: &[u8] = b"\x04\x08IC:\x0dMyString\"\x0ahello\x06:\x06ET";

    // h = {"a" => 1}; h.instance_variable_set(:@meta, 2)
    // an Instance wrapping a Hash whose key is itself an ivar'd string
    const HASH_WITH_IVARS: &[u8] = b"\x04\x08I{\x06I\"\x06a\x06:\x06ETi\x06\x06:\x0a@metai\x07";

    // Marshal.dump(Item.new.extend(Magic)) where Item has @name = "x"
    // an Extended wrapping an Object whose field is an ivar'd string
    const EXTENDED_ITEM: &[u8] = b"\x04\x08e:\x0aMagico:\x09Item\x06:\x0a@nameI\"\x06x\x06:\x06ET";

    #[test]
    fn nested_wrappers_as_nested_tuples() {
        // each wrapper layer is its own sequence: Instance(UserClass(String))
        let result: Ivar<(&str, &str), HashMap<&str, bool>> =
            de_from_ruby(SUBCLASSED_STRING).unwrap();
        let (inner, class) = result.inner;
        assert_eq!(inner, "hello");
        assert_eq!(class, "MyString");
        assert!(result.ivars["E"]);
    }

    #[test]
    fn nested_wrappers_unwrap_with_nested_transparent() {
        let result: Transparent<Transparent<&str>> = de_from_ruby(SUBCLASSED_STRING).unwrap();
        assert_eq!(result.0.0, "hello");
    }

    #[test]
    fn nested_wrappers_flatten_fully_opinionated() {
        let result: &str =
            from_bytes_with_config(SUBCLASSED_STRING, DeserializerConfig::opinionated()).unwrap();
        assert_eq!(result, "hello");
    }

    #[test]
    fn nested_wrapper_flags_compose_independently() {
        // only the ivar layer is flattened, the user class layer keeps its sequence shape
        let config = DeserializerConfig::new().with_ivar_as_inner(true);
        let (inner, class): (&str, &str) =
            from_bytes_with_config(SUBCLASSED_STRING, config).unwrap();
        assert_eq!(inner, "hello");
        assert_eq!(class, "MyString");
    }

    #[test]
    fn chained_wrappers_register_one_object() {
        // Marshal.dump([s, s]) where s = MyString.new("hello"): the whole I+C+" construct is
        // a single object table entry, so the link must replay all three layers
        let bytes = b"\x04\x08[\x07IC:\x0dMyString\"\x0ahello\x06:\x06ET@\x06";
        let result: Vec<Transparent<Transparent<&str>>> = de_from_ruby(bytes).unwrap();
        assert_eq!(result[0].0.0, "hello");
        assert_eq!(result[1].0.0, "hello");
    }

    #[test]
    fn hash_with_ivars() {
        let result: Ivar<HashMap<Transparent<String>, i32>, HashMap<&str, i32>> =
            de_from_ruby(HASH_WITH_IVARS).unwrap();
        assert_eq!(result.inner[&Transparent("a".to_string())], 1);
        assert_eq!(result.ivars["@meta"], 2);
    }

    #[test]
    fn hash_with_ivars_opinionated() {
        let result: HashMap<&str, i32> =
            from_bytes_with_config(HASH_WITH_IVARS, DeserializerConfig::opinionated()).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result["a"], 1);
    }

    #[test]
    fn extended_object_with_ivar_string_field() {
        let (item, module): (RbObject<HashMap<&str, Ivar<&str>>, &str>, &str) =
            de_from_ruby(EXTENDED_ITEM).unwrap();
        assert_eq!(module, "Magic");
        assert_eq!(item.class, "Item");
        assert_eq!(item.fields["@name"].inner, "x");
    }

    #[test]
    fn extended_object_flattens_opinionated() {
        #[derive(Debug, Deserialize, PartialEq)]
        struct Item<'a> {
            #[serde(rename = "@name")]
            name: &'a str,
        }
        let result: Item =
            from_bytes_with_config(EXTENDED_ITEM, DeserializerConfig::opinionated()).unwrap();
        assert_eq!(result, Item { name: "x" });
    }

    #[test]
    fn user_marshal_of_ivar_strings() {
        // U:\x08Foo wrapping ["hello", "world"]; the second string's :E ivar is emitted as a
        // symbol link (;\x06) the way Ruby would
        let bytes = b"\x04\x08U:\x08Foo[\x07I\"\x0ahello\x06:\x06ETI\"\x0aworld\x06;\x06T";
        let (values, class): (Vec<Ivar<&str>>, &str) = de_from_ruby(bytes).unwrap();
        assert_eq!(class, "Foo");
        assert_eq!(values[0].inner, "hello");
        assert_eq!(values[1].inner, "world");
    }

    // o:\x06A {@b => o:\x06B {@c => o:\x06C {@x => 1}}}
    const NESTED_OBJECTS: &[u8] =
        b"\x04\x08o:\x06A\x06:\x07@bo:\x06B\x06:\x07@co:\x06C\x06:\x07@xi\x06";

    #[test]
    fn objects_nested_three_deep() {
        #[derive(Debug, Deserialize)]
        struct AFields {
            #[serde(rename = "@b")]
            b: RbObject<BFields>,
        }
        #[derive(Debug, Deserialize)]
        struct BFields {
            #[serde(rename = "@c")]
            c: RbObject<CFields>,
        }
        #[derive(Debug, Deserialize)]
        struct CFields {
            #[serde(rename = "@x")]
            x: i32,
        }

        let result: RbObject<AFields> = de_from_ruby(NESTED_OBJECTS).unwrap();
        assert_eq!(result.fields.b.fields.c.fields.x, 1);
    }

    #[test]
    fn objects_nested_three_deep_opinionated() {
        #[derive(Debug, Deserialize)]
        struct A {
            #[serde(rename = "@b")]
            b: B,
        }
        #[derive(Debug, Deserialize)]
        struct B {
            #[serde(rename = "@c")]
            c: C,
        }
        #[derive(Debug, Deserialize)]
        struct C {
            #[serde(rename = "@x")]
            x: i32,
        }

        let result: A =
            from_bytes_with_config(NESTED_OBJECTS, DeserializerConfig::opinionated()).unwrap();
        assert_eq!(result.b.c.x, 1);
    }

    #[test]
    fn tuple_shapes_put_the_useful_value_first() {
        // every wrapper sequence leads with its useful value, so Transparent grabs the fields
        // of an Object the same way it grabs the inner value of an Instance
        let object: Transparent<HashMap<&str, i32>> = de_from_ruby(OBJECT_PT).unwrap();
        assert_eq!(object.0["@x"], 10);

        // U:\x08Foo[\x06i\x06 — UserMarshal of [1]
        let user_marshal: Transparent<Vec<i32>> =
            de_from_ruby(b"\x04\x08U:\x08Foo[\x06i\x06").unwrap();
        assert_eq!(user_marshal.0, vec![1]);
    }
}
