use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

use serde::de::{Deserializer, SeqAccess, Visitor};

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

/// Type for deserializing Ruby regex values.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RbRegex<P = String> {
    pub pattern: P,
    pub flags: u8,
}

impl<P> Deref for RbRegex<P> {
    type Target = P;
    fn deref(&self) -> &P {
        &self.pattern
    }
}

impl<P> DerefMut for RbRegex<P> {
    fn deref_mut(&mut self) -> &mut P {
        &mut self.pattern
    }
}

impl<'de, P> serde::Deserialize<'de> for RbRegex<P>
where
    P: serde::Deserialize<'de>,
{
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RbRegexVisitor<P>(PhantomData<P>);

        impl<'de, P> Visitor<'de> for RbRegexVisitor<P>
        where
            P: serde::Deserialize<'de>,
        {
            type Value = RbRegex<P>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a Ruby Regex (2-element sequence: pattern, flags)")
            }

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let pattern = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                let flags = seq
                    .next_element()?
                    .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                Ok(RbRegex { pattern, flags })
            }
        }

        deserializer.deserialize_tuple(2, RbRegexVisitor(PhantomData))
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
