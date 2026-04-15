use std::str::Utf8Error;

use num_bigint::BigInt;

use super::MAX_REF_DEPTH;
use crate::{marshal::ParseError, types::value::MarshalValue};

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
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error("invalid UTF-8: {0}")]
    InvalidUtf8(#[from] Utf8Error),
    #[error("invalid symbol index {0}")]
    InvalidSymbolIndex(usize),
    #[error("invalid object ref {0}")]
    InvalidObjectRef(usize),
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
    #[error("cannot deserialize {0} in self-describing mode")]
    UnsupportedType(&'static str),
    #[error(
        "cyclic or too-deep object reference chain (>{MAX_REF_DEPTH} hops) if you are hitting this one on data you know is good go open a github issue and cry at the maintainer plz"
    )]
    CyclicRef,
}

impl From<ParseError> for MarshalDeserializeError {
    fn from(e: ParseError) -> Self {
        ErrorKind::Parse(e).into()
    }
}

impl serde_core::de::Error for MarshalDeserializeError {
    fn custom<T: std::fmt::Display>(msg: T) -> Self {
        ErrorKind::Message(msg.to_string().into_boxed_str()).into()
    }
}

/// Shorthand for creating a [`ErrorKind::TypeMismatch`] error from a [`MarshalValue`].
pub(crate) fn type_mismatch(
    expected: &'static str,
    got: &MarshalValue<'_>,
) -> MarshalDeserializeError {
    ErrorKind::TypeMismatch {
        expected,
        got: got.as_snake_case(),
    }
    .into()
}
