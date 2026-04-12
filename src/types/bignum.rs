use std::num::TryFromIntError;

use num_bigint::{BigInt, Sign};

use crate::{
    cursor::{Cursor, FromCursor, TryFromCursor},
    types::fixnum::FixNumLen,
};

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("{} should be either '+' or '-'", *byte as char)]
pub struct IncorrectSign {
    byte: u8,
}

impl IncorrectSign {
    fn new(byte: u8) -> Self {
        Self { byte }
    }
}

impl TryFromCursor<'_> for Sign {
    type Error = IncorrectSign;

    fn try_from_cursor(cursor: &mut Cursor<'_>) -> Option<Result<Self, Self::Error>> {
        u8::from_cursor(cursor).map(|b| match b {
            b'+' => Ok(Sign::Plus),
            b'-' => Ok(Sign::Minus),
            other => Err(IncorrectSign::new(other)),
        })
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{kind}")]
pub struct ParseBigIntError {
    kind: ParseBigIntErrorKind,
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
enum ParseBigIntErrorKind {
    #[error("{0}")]
    Sign(#[from] IncorrectSign),
    #[error("unable to cast fixnum length of bignum to usize")]
    LenTooLong(#[from] TryFromIntError),
}

macro_rules! tri_opt {
    ($expr:expr) => {
        match $expr {
            Some(Ok(val)) => val,
            None => return None,
            Some(Err(e)) => {
                return Some(Err(ParseBigIntError {
                    kind: ParseBigIntErrorKind::from(e),
                }))
            }
        }
    };
}

impl TryFromCursor<'_> for num_bigint::BigInt {
    type Error = ParseBigIntError;

    fn try_from_cursor(cursor: &mut Cursor<'_>) -> Option<Result<Self, Self::Error>> {
        let sign = tri_opt!(cursor.try_take::<Sign>());
        let len = tri_opt!(cursor.try_take::<FixNumLen>()).into_inner() * 2;
        let bignum_bytes = cursor.take_n(len)?;

        Some(Ok(BigInt::from_bytes_le(sign, bignum_bytes)))
    }
}
