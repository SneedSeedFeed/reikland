use num_bigint::Sign;

use crate::cursor::{Cursor, FromCursor, TryFromCursor};

#[derive(Debug, Clone, Copy, snafu::Snafu)]
#[snafu(display("{} should be either '+' or '-'", *byte as char))]
pub struct IncorrectSign {
    byte: u8,
}

impl IncorrectSign {
    fn new(byte: u8) -> Self {
        Self { byte }
    }
}

impl TryFromCursor for Sign {
    type Error = IncorrectSign;

    fn try_from_cursor(cursor: &mut Cursor<'_>) -> Option<Result<Self, Self::Error>> {
        u8::from_cursor(cursor).map(|b| match b {
            b'+' => Ok(Sign::Plus),
            b'-' => Ok(Sign::Minus),
            other => Err(IncorrectSign::new(other)),
        })
    }
}
