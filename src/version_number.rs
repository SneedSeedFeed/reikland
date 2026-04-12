use crate::cursor::{Cursor, TryFromCursor};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VersionNumber {
    pub major: u8,
    pub minor: u8,
}

impl std::fmt::Display for VersionNumber {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl VersionNumber {
    pub(crate) fn can_read(&self) -> bool {
        self.major == 4 && self.minor <= 8
    }

    pub fn major(&self) -> u8 {
        self.major
    }

    pub fn minor(&self) -> u8 {
        self.minor
    }
}

// not sure if i should give it this impl since it's not a "real" value
impl TryFromCursor<'_> for VersionNumber {
    type Error = std::convert::Infallible; // could make invalid version numbers an error?

    fn try_from_cursor(cursor: &mut Cursor<'_>) -> Option<Result<Self, Self::Error>> {
        let [major, minor] = cursor.take_const::<2>().copied()?;
        Some(Ok(VersionNumber { major, minor }))
    }
}
