use crate::{
    cursor::{
        Cursor, FromCursor, TryFromCursor,
        object_table::{ObjectIdx, ObjectRefIdx, ObjectTable},
        symbol_table::{SymbolIdx, SymbolTable},
    },
    types::{
        bignum::ParseBigIntError,
        fixnum::{FixNum, FixNumLen},
        float::{ParseRbFloatError, RbFloat},
        string::{ParseRbStrError, RbStr},
        type_byte::{InvalidTypeByte, MarshalTypeByte},
        value::MarshalValue,
    },
    version_number::VersionNumber,
};

/// The fully parsed output of a Ruby marshal byte stream.
///
/// Symbols and objects are stored in flat tables, referenced by [`SymbolIdx`] and [`ObjectIdx`] respectively.
#[derive(Debug)]
pub struct MarshalData<'a> {
    pub version: VersionNumber,
    pub symbols: SymbolTable<'a>,
    pub objects: ObjectTable<'a>,
    pub root: ObjectIdx,
}

impl<'a> MarshalData<'a> {
    pub fn symbol(&self, idx: SymbolIdx) -> Option<&'a RbStr> {
        self.symbols.get(idx)
    }

    pub fn object(&self, idx: ObjectIdx) -> &MarshalValue<'a> {
        &self.objects[idx]
    }

    pub fn root(&self) -> &MarshalValue<'a> {
        self.object(self.root)
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{kind}")]
pub struct ParseError {
    #[from]
    kind: ParseErrorKind,
}

#[derive(Debug, thiserror::Error)]
enum ParseErrorKind {
    #[error("We do not support version {0}")]
    VersionNumber(VersionNumber),
    #[error("unexpected end of input")]
    UnexpectedEof,
    #[error("invalid type byte: {0}")]
    InvalidTypeByte(#[from] InvalidTypeByte),
    #[error("failed to parse string: {0}")]
    String(#[from] ParseRbStrError),
    #[error("failed to parse float: {0}")]
    Float(#[from] ParseRbFloatError),
    #[error("failed to parse bignum: {0}")]
    Bignum(#[from] ParseBigIntError),
    #[error("failed to parse element count")]
    InvalidLen(#[from] std::num::TryFromIntError),
    #[error("expected symbol in this position but got {0}")]
    ExpectedSymbol(MarshalTypeByte),
}

/// Parse a complete marshal byte stream into [`MarshalData`].
pub fn parse<'a>(input: &'a [u8]) -> Result<MarshalData<'a>, ParseError> {
    let mut cursor = Cursor::new(input);

    let version: VersionNumber = cursor.take().ok_or(ParseErrorKind::UnexpectedEof)?;
    if !version.can_read() {
        return Err(ParseErrorKind::VersionNumber(version).into());
    }
    let root = parse_value(&mut cursor)?;
    let (symbols, objects) = cursor.into_tables();

    Ok(MarshalData {
        version,
        symbols,
        objects,
        root,
    })
}

/// Parse a symbol that may be either a [`MarshalTypeByte::Symbol`] (`:`) or a [`MarshalTypeByte::SymbolLink`] (`;`), returning its [`SymbolIdx`]
fn parse_symbol<'a>(cursor: &mut Cursor<'a>) -> Result<SymbolIdx, ParseError> {
    let type_byte = try_take(cursor)?;

    match type_byte {
        MarshalTypeByte::Symbol => {
            let bytes: &'a RbStr = try_take(cursor)?;
            let idx = cursor.push_symbol(bytes);
            Ok(idx)
        }
        MarshalTypeByte::SymbolLink => {
            let fixnum: FixNum = take(cursor)?;
            Ok(SymbolIdx::new(fixnum.inner() as usize))
        }
        _ => Err(ParseError::from(ParseErrorKind::ExpectedSymbol(type_byte))),
    }
}

/// Parse instance variables: a count followed by symbol-value pairs.
fn parse_ivars<'a>(cursor: &mut Cursor<'a>) -> Result<Vec<(SymbolIdx, ObjectIdx)>, ParseError> {
    let len: FixNumLen = try_take(cursor)?;

    let mut ivars = Vec::with_capacity(len.inner());
    for _ in 0..len.inner() {
        let sym = parse_symbol(cursor)?;
        let val = parse_value(cursor)?;
        ivars.push((sym, val));
    }

    Ok(ivars)
}

/// Parse a single value from the stream, push it into the object store, and return its [`ObjectIdx`].
fn parse_value<'a>(cursor: &mut Cursor<'a>) -> Result<ObjectIdx, ParseError> {
    let type_byte = try_take(cursor)?;

    match type_byte {
        MarshalTypeByte::Nil => Ok(cursor.push_object(MarshalValue::Nil)),
        MarshalTypeByte::True => Ok(cursor.push_object(MarshalValue::True)),
        MarshalTypeByte::False => Ok(cursor.push_object(MarshalValue::False)),

        MarshalTypeByte::Fixnum => {
            let fixnum: FixNum = take(cursor)?;
            Ok(cursor.push_object(MarshalValue::Fixnum(fixnum.inner())))
        }

        MarshalTypeByte::Symbol => {
            let bytes: &'a RbStr = try_take(cursor)?;
            let _sym_idx = cursor.push_symbol(bytes);
            Ok(cursor.push_object(MarshalValue::Symbol(bytes)))
        }

        MarshalTypeByte::SymbolLink => {
            let fixnum: FixNum = take(cursor)?;
            let sym_idx = SymbolIdx::new(fixnum.inner() as usize);
            Ok(cursor.push_object(MarshalValue::SymbolLink(sym_idx)))
        }

        MarshalTypeByte::ObjectReference => {
            let fixnum: FixNum = take(cursor)?;
            let ref_idx = ObjectRefIdx::new(fixnum.inner() as usize);
            Ok(cursor.push_object(MarshalValue::ObjectRef(ref_idx)))
        }

        MarshalTypeByte::String => {
            let bytes: &'a RbStr = try_take(cursor)?;
            let obj_idx = cursor.push_object(MarshalValue::String(bytes));
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }

        MarshalTypeByte::Float => {
            let float: RbFloat = try_take(cursor)?;
            let obj_idx = cursor.push_object(MarshalValue::Float(float.inner()));
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }

        MarshalTypeByte::Bignum => {
            let bignum = try_take(cursor)?;
            let obj_idx = cursor.push_object(MarshalValue::Bignum(bignum));
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }

        MarshalTypeByte::Array => {
            let len: FixNumLen = try_take(cursor)?;

            let mut elements = Vec::with_capacity(len.inner());
            for _ in 0..len.inner() {
                elements.push(parse_value(cursor)?);
            }

            let obj_idx = cursor.push_object(MarshalValue::Array(elements));
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }

        MarshalTypeByte::Hash => {
            let len: FixNumLen = try_take(cursor)?;

            let mut pairs = Vec::with_capacity(len.inner());
            for _ in 0..len.inner() {
                let key = parse_value(cursor)?;
                let val = parse_value(cursor)?;
                pairs.push((key, val));
            }

            let obj_idx = cursor.push_object(MarshalValue::Hash(pairs));
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }

        MarshalTypeByte::HashDefault => {
            let len: FixNumLen = try_take(cursor)?;

            let mut pairs = Vec::with_capacity(len.inner());
            for _ in 0..len.inner() {
                let key = parse_value(cursor)?;
                let val = parse_value(cursor)?;
                pairs.push((key, val));
            }

            let default = parse_value(cursor)?;
            let obj_idx = cursor.push_object(MarshalValue::HashDefault { pairs, default });
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }

        MarshalTypeByte::RegularExpression => {
            let pattern: &'a RbStr = try_take(cursor)?;
            let flags = cursor.take_1().ok_or(ParseErrorKind::UnexpectedEof)?;

            let obj_idx = cursor.push_object(MarshalValue::Regex { pattern, flags });
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }

        MarshalTypeByte::Instance => {
            let inner = parse_value(cursor)?;
            let ivars = parse_ivars(cursor)?;

            let obj_idx = cursor.push_object(MarshalValue::Instance { inner, ivars });
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }

        MarshalTypeByte::Extended => {
            let module = parse_symbol(cursor)?;
            let inner = parse_value(cursor)?;

            let obj_idx = cursor.push_object(MarshalValue::Extended { module, inner });
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }

        MarshalTypeByte::Object => {
            let class = parse_symbol(cursor)?;
            let ivars = parse_ivars(cursor)?;

            let obj_idx = cursor.push_object(MarshalValue::Object { class, ivars });
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }

        MarshalTypeByte::Struct => {
            let name = parse_symbol(cursor)?;
            let len: FixNumLen = try_take(cursor)?;

            let mut members = Vec::with_capacity(len.inner());
            for _ in 0..len.inner() {
                let sym = parse_symbol(cursor)?;
                let val = parse_value(cursor)?;
                members.push((sym, val));
            }

            let obj_idx = cursor.push_object(MarshalValue::Struct { name, members });
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }

        MarshalTypeByte::Class => {
            let bytes: &'a RbStr = try_take(cursor)?;
            let obj_idx = cursor.push_object(MarshalValue::Class(bytes));
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }

        MarshalTypeByte::Module => {
            let bytes: &'a RbStr = try_take(cursor)?;
            let obj_idx = cursor.push_object(MarshalValue::Module(bytes));
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }

        MarshalTypeByte::ClassOrModule => {
            let bytes: &'a RbStr = try_take(cursor)?;
            let obj_idx = cursor.push_object(MarshalValue::ClassOrModule(bytes));
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }

        MarshalTypeByte::Data => {
            let class = parse_symbol(cursor)?;
            let inner = parse_value(cursor)?;

            let obj_idx = cursor.push_object(MarshalValue::Data { class, inner });
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }

        MarshalTypeByte::UserDefined => {
            let class = parse_symbol(cursor)?;
            let len: FixNumLen = try_take(cursor)?;
            let data = cursor
                .take_n(len.inner())
                .ok_or(ParseErrorKind::UnexpectedEof)?;

            let obj_idx = cursor.push_object(MarshalValue::UserDefined { class, data });
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }

        MarshalTypeByte::UserMarshal => {
            let class = parse_symbol(cursor)?;
            let inner = parse_value(cursor)?;

            let obj_idx = cursor.push_object(MarshalValue::UserMarshal { class, inner });
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }

        MarshalTypeByte::UserString => {
            let class = parse_symbol(cursor)?;
            let inner = parse_value(cursor)?;

            let obj_idx = cursor.push_object(MarshalValue::UserString { class, inner });
            cursor.push_object_ref(obj_idx);
            Ok(obj_idx)
        }
    }
}

/// take an infallible value or return [`ParseErrorKind::UnexpectedEof`].
fn take<'a, T: FromCursor<'a>>(cursor: &mut Cursor<'a>) -> Result<T, ParseError> {
    cursor
        .take()
        .ok_or(ParseErrorKind::UnexpectedEof)
        .map_err(ParseError::from)
}

/// take a fallible value, mapping [`None`] to EOF and the Err through [`From::from`]
fn try_take<'a, T>(cursor: &mut Cursor<'a>) -> Result<T, ParseError>
where
    T: TryFromCursor<'a>,
    ParseErrorKind: From<<T as TryFromCursor<'a>>::Error>,
{
    cursor
        .try_take::<T>()
        .ok_or(ParseErrorKind::UnexpectedEof)?
        .map_err(ParseErrorKind::from)
        .map_err(ParseError::from)
}
