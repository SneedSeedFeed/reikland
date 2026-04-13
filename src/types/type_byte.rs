use crate::cursor::{Cursor, TryFromCursor};

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum MarshalTypeByte {
    True = b'T',
    False = b'F',
    Nil = b'0',
    Fixnum = b'i',
    Symbol = b':',
    SymbolLink = b';',
    ObjectReference = b'@',
    Instance = b'I',
    Extended = b'e',
    Array = b'[',
    Bignum = b'l',
    Class = b'c',
    Module = b'm',
    Data = b'd',
    Float = b'f',
    Hash = b'{',
    HashDefault = b'}',
    Object = b'o',
    RegularExpression = b'/',
    String = b'"',
    Struct = b'S',
    UserString = b'C',
    UserDefined = b'u',
    UserMarshal = b'U',
    ClassOrModule = b'M',
}

impl std::fmt::Display for MarshalTypeByte {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (*self as u8 as char).fmt(f)
    }
}

impl MarshalTypeByte {
    pub fn try_from_u8(byte: u8) -> Option<Self> {
        match byte {
            b'T' => Some(Self::True),
            b'F' => Some(Self::False),
            b'0' => Some(Self::Nil),
            b'i' => Some(Self::Fixnum),
            b':' => Some(Self::Symbol),
            b';' => Some(Self::SymbolLink),
            b'@' => Some(Self::ObjectReference),
            b'I' => Some(Self::Instance),
            b'e' => Some(Self::Extended),
            b'[' => Some(Self::Array),
            b'l' => Some(Self::Bignum),
            b'c' => Some(Self::Class),
            b'm' => Some(Self::Module),
            b'd' => Some(Self::Data),
            b'f' => Some(Self::Float),
            b'{' => Some(Self::Hash),
            b'}' => Some(Self::HashDefault),
            b'o' => Some(Self::Object),
            b'/' => Some(Self::RegularExpression),
            b'"' => Some(Self::String),
            b'S' => Some(Self::Struct),
            b'C' => Some(Self::UserString),
            b'u' => Some(Self::UserDefined),
            b'U' => Some(Self::UserMarshal),
            b'M' => Some(Self::ClassOrModule),
            _ => None,
        }
    }
}

impl TryFrom<u8> for MarshalTypeByte {
    type Error = InvalidTypeByte;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        MarshalTypeByte::try_from_u8(value).ok_or(InvalidTypeByte { byte: value })
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("{} is not a valid marshal type byte", *byte as char)]
pub struct InvalidTypeByte {
    byte: u8,
}

impl TryFromCursor<'_> for MarshalTypeByte {
    type Error = InvalidTypeByte;

    fn try_from_cursor(cursor: &mut Cursor<'_>) -> Option<Result<Self, Self::Error>> {
        cursor.take_1().map(TryFrom::try_from)
    }
}
