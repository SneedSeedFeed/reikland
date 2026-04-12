use crate::{
    cursor::{Cursor, TryFromCursor},
    types::string::RbStr,
};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct RbRegexStr<'a> {
    flags: u8,
    pattern: &'a RbStr,
}

impl<'a> RbRegexStr<'a> {
    // i have no desire to flesh this type out like RbStr so this is all it's getting
    pub fn inner(self) -> (u8, &'a RbStr) {
        (self.flags, self.pattern)
    }
}

impl<'a> TryFromCursor<'a> for RbRegexStr<'a> {
    type Error = <&'a RbStr as TryFromCursor<'a>>::Error;

    fn try_from_cursor(cursor: &mut Cursor<'a>) -> Option<Result<Self, Self::Error>> {
        let pattern = match cursor.try_take::<&RbStr>()? {
            Ok(pat) => pat,
            Err(e) => return Some(Err(e)),
        };

        let flags = cursor.take_1()?;

        Some(Ok(RbRegexStr { flags, pattern }))
    }
}
