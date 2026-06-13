use std::str::Utf8Error;

use num_bigint::BigInt;

use super::MAX_REF_DEPTH;
use crate::{
    types::{
        bignum::ParseBigIntError,
        float::ParseRbFloatError,
        string::ParseRbStrError,
        type_byte::{InvalidTypeByte, MarshalTypeByte},
    },
    version_number::VersionNumber,
};

#[derive(Debug, thiserror::Error)]
#[error("{kind}")]
pub struct MarshalDeserializeError {
    #[from]
    kind: ErrorKind,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum ErrorKind {
    #[error("{0}")]
    Message(Box<str>),
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
    #[error("expected symbol in this position but got {}", .0.type_name())]
    ExpectedSymbol(MarshalTypeByte),
    #[error("invalid UTF-8: {0}")]
    InvalidUtf8(#[from] Utf8Error),
    #[error("invalid symbol link {0}")]
    InvalidSymbolLink(i32),
    #[error("invalid object ref {0}")]
    InvalidObjectRef(i32),
    #[error("expected {expected}, got {got}")]
    TypeMismatch {
        expected: &'static str,
        got: &'static str,
    },
    #[error("{target_type} overflow from i32 '{value}'")]
    IntegerOverflowI32 {
        target_type: &'static str,
        value: i32,
    },
    #[error("{target_type} overflow from bigint '{value}'")]
    IntegerOverflowBigInt {
        target_type: &'static str,
        value: BigInt,
    },
    #[error("bignum too large for any integer type")]
    BignumTooLarge,
    #[error("expected single char, got string of len {len}")]
    ExpectedSingleChar { len: usize },
    #[error("expected an enum as a single-entry hash, got a hash of {0} entries")]
    EnumHashLen(usize),
    #[error(
        "cyclic or too-deep object reference chain (>{MAX_REF_DEPTH} hops) if you are hitting this one on data you know is good go open a github issue and cry at the maintainer plz"
    )]
    CyclicRef,
}

impl serde_core::de::Error for MarshalDeserializeError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        ErrorKind::Message(msg.to_string().into_boxed_str()).into()
    }
}

/// Shorthand for creating a [`ErrorKind::TypeMismatch`] error from a [`MarshalTypeByte`].
pub(crate) fn type_mismatch(
    expected: &'static str,
    got: MarshalTypeByte,
) -> MarshalDeserializeError {
    ErrorKind::TypeMismatch {
        expected,
        got: got.type_name(),
    }
    .into()
}
