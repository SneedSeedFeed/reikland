use crate::{
    cursor::{Cursor, TryFromCursor},
    types::string::{ParseRbStrError, RbStr},
};

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Default)]
pub struct RbFloat(f64);

impl RbFloat {
    pub fn new(float: f64) -> Self {
        Self(float)
    }

    pub fn inner(self) -> f64 {
        self.0
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{kind}")]
pub struct ParseRbFloatError {
    #[from]
    kind: ParseRbFloatErrorKind,
}

#[derive(Debug, Clone, thiserror::Error)]
enum ParseRbFloatErrorKind {
    #[error("failed to read float: {0}")]
    Str(#[from] ParseRbStrError),
    #[error("failed to parse float: {0}")]
    Parse(#[from] lexical_parse_float::Error), // it was either add this dep OR convert to utf8 just to parse a float
}

// insert systemshock pun
macro_rules! tri_opt {
    ($expr:expr) => {
        match $expr {
            Some(Ok(val)) => val,
            None => return None,
            Some(Err(e)) => {
                return Some(Err(ParseRbFloatError {
                    kind: ParseRbFloatErrorKind::from(e),
                }))
            }
        }
    };
}

impl TryFromCursor<'_> for RbFloat {
    type Error = ParseRbFloatError;

    fn try_from_cursor(cursor: &mut Cursor<'_>) -> Option<Result<Self, Self::Error>> {
        let bytes = tri_opt!(cursor.try_take::<&RbStr>()).as_slice();

        match bytes {
            b"nan" => Some(Ok(RbFloat(f64::NAN))),
            b"inf" => Some(Ok(RbFloat(f64::INFINITY))),
            b"-inf" => Some(Ok(RbFloat(f64::NEG_INFINITY))),
            other => Some(
                lexical_parse_float::FromLexical::from_lexical(other)
                    .map(RbFloat)
                    .map_err(ParseRbFloatErrorKind::from)
                    .map_err(ParseRbFloatError::from),
            ),
        }
    }
}
