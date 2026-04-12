use std::num::TryFromIntError;

use crate::cursor::{Cursor, FromCursor, TryFromCursor};

/// Ruby fixnum
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct FixNum(i32);

impl FixNum {
    pub fn new(val: i32) -> Self {
        Self(val)
    }

    pub fn into_inner(self) -> i32 {
        self.0
    }
}

impl From<i32> for FixNum {
    fn from(value: i32) -> Self {
        Self::new(value)
    }
}

impl TryFromCursor<'_> for FixNum {
    type Error = std::convert::Infallible;

    fn try_from_cursor(cursor: &mut Cursor<'_>) -> Option<Result<Self, Self::Error>> {
        match u8::from_cursor(cursor)? {
            0x00 => Some(Ok(FixNum(0))),
            0x01 => {
                let first_byte = cursor.take()?;
                Some(Ok(FixNum(i32::from_le_bytes([first_byte, 0, 0, 0]))))
            }
            0xff => {
                let first_byte = cursor.take()?;
                Some(Ok(FixNum(i32::from_le_bytes([
                    first_byte, 0xff, 0xff, 0xff,
                ]))))
            }
            0x02 => {
                let [first_byte, second_byte] = cursor.take_const::<2>().copied()?;
                Some(Ok(FixNum(i32::from_le_bytes([
                    first_byte,
                    second_byte,
                    0,
                    0,
                ]))))
            }
            0xfe => {
                let [first_byte, second_byte] = cursor.take_const::<2>().copied()?;
                Some(Ok(FixNum(i32::from_le_bytes([
                    first_byte,
                    second_byte,
                    0xff,
                    0xff,
                ]))))
            }
            0x03 => {
                let [first_byte, second_byte, third_byte] = cursor.take_const::<3>().copied()?;
                Some(Ok(FixNum(i32::from_le_bytes([
                    first_byte,
                    second_byte,
                    third_byte,
                    0,
                ]))))
            }
            0xfd => {
                let [first_byte, second_byte, third_byte] = cursor.take_const::<3>().copied()?;
                Some(Ok(FixNum(i32::from_le_bytes([
                    first_byte,
                    second_byte,
                    third_byte,
                    0xff,
                ]))))
            }
            0x04 | 0xfc => Some(Ok(FixNum(cursor.take::<i32>()?))),
            x => {
                let signed = x.cast_signed();
                Some(Ok(FixNum((signed - (signed.signum() * 5)) as i32)))
            }
        }
    }
}

/// Ruby fixnum, converted into a usize
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FixNumLen(usize);

impl FixNumLen {
    pub fn new(len: usize) -> Self {
        Self(len)
    }

    pub fn into_inner(self) -> usize {
        self.0
    }
}

impl TryFromCursor<'_> for FixNumLen {
    type Error = TryFromIntError;

    fn try_from_cursor(cursor: &mut Cursor<'_>) -> Option<Result<Self, Self::Error>> {
        i32::try_from_cursor(cursor)
            .map(Result::unwrap)
            .map(usize::try_from)
            .map(|o| o.map(FixNumLen))
    }
}
