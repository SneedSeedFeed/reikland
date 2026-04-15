use std::{
    num::TryFromIntError,
    ops::{Deref, DerefMut},
};

use crate::{
    cursor::{Cursor, TryFromCursor},
    types::fixnum::FixNumLen,
};

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct RbStr {
    inner: [u8],
}

impl RbStr {
    pub fn from_slice(slice: &[u8]) -> &RbStr {
        slice.into()
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.inner
    }
}

// This implementation is ripped out of the source code for `ByteStr` on nightly
impl std::fmt::Display for RbStr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        fn fmt_nopad(this: &RbStr, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            for chunk in this.utf8_chunks() {
                f.write_str(chunk.valid())?;
                if !chunk.invalid().is_empty() {
                    f.write_str("\u{FFFD}")?;
                }
            }
            Ok(())
        }

        let Some(align) = f.align() else {
            return fmt_nopad(self, f);
        };
        let nchars: usize = self
            .utf8_chunks()
            .map(|chunk| {
                chunk.valid().chars().count() + if chunk.invalid().is_empty() { 0 } else { 1 }
            })
            .sum();
        let padding = f.width().unwrap_or(0).saturating_sub(nchars);
        let fill = f.fill();
        let (lpad, rpad) = match align {
            std::fmt::Alignment::Left => (0, padding),
            std::fmt::Alignment::Right => (padding, 0),
            std::fmt::Alignment::Center => {
                let half = padding / 2;
                (half, half + padding % 2)
            }
        };
        for _ in 0..lpad {
            write!(f, "{fill}")?;
        }
        fmt_nopad(self, f)?;
        for _ in 0..rpad {
            write!(f, "{fill}")?;
        }

        Ok(())
    }
}

impl AsRef<[u8]> for RbStr {
    fn as_ref(&self) -> &[u8] {
        &self.inner
    }
}

impl AsMut<[u8]> for RbStr {
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.inner
    }
}

impl Deref for RbStr {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl DerefMut for RbStr {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl From<&str> for &RbStr {
    fn from(value: &str) -> Self {
        value.as_bytes().into()
    }
}

impl From<&[u8]> for &RbStr {
    fn from(value: &[u8]) -> Self {
        let ptr = std::ptr::from_ref(value);
        let cast = ptr as *const RbStr;
        // Safety: RbStr is repr(transparent) thus should be exactly the same as [u8] in every way. Also miri didn't cry when i ran `cargo +nightly miri test` and i trust miri with my life.
        unsafe { &*cast }
    }
}

impl<'a> TryFrom<&'a RbStr> for &'a str {
    type Error = std::str::Utf8Error;

    fn try_from(value: &RbStr) -> Result<&str, Self::Error> {
        std::str::from_utf8(value)
    }
}

/// Owned counterpart of [`RbStr`]. Holds raw bytes from a Ruby string without assuming any particular encoding
/// If encoding is needed [`Encoding`][crate::deserializer_types::ivar::Encoding] or [`WithEncoding`][crate::deserializer_types::ivar::WithEncoding] to discover the encoding if you need it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct RbString {
    inner: Vec<u8>,
}

impl RbString {
    pub fn into_vec(self) -> Vec<u8> {
        self.inner
    }
}

impl Deref for RbString {
    type Target = RbStr;

    fn deref(&self) -> &RbStr {
        RbStr::from_slice(&self.inner)
    }
}

impl DerefMut for RbString {
    fn deref_mut(&mut self) -> &mut RbStr {
        let ptr = std::ptr::from_mut(self.inner.as_mut_slice());
        let cast = ptr as *mut RbStr;
        // Safety: RbStr is repr(transparent) over [u8]
        unsafe { &mut *cast }
    }
}

impl From<Vec<u8>> for RbString {
    fn from(v: Vec<u8>) -> Self {
        RbString { inner: v }
    }
}

impl From<RbString> for Vec<u8> {
    fn from(s: RbString) -> Self {
        s.inner
    }
}

impl AsRef<[u8]> for RbString {
    fn as_ref(&self) -> &[u8] {
        &self.inner
    }
}

impl AsRef<RbStr> for RbString {
    fn as_ref(&self) -> &RbStr {
        self
    }
}

impl std::fmt::Display for RbString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <RbStr as std::fmt::Display>::fmt(self, f)
    }
}

impl<'de> serde::Deserialize<'de> for RbString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct RbStringVisitor;

        impl<'de> serde::de::Visitor<'de> for RbStringVisitor {
            type Value = RbString;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a byte string")
            }

            fn visit_bytes<E: serde::de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                Ok(RbString { inner: v.to_vec() })
            }

            fn visit_borrowed_bytes<E: serde::de::Error>(
                self,
                v: &'de [u8],
            ) -> Result<Self::Value, E> {
                Ok(RbString { inner: v.to_vec() })
            }

            fn visit_byte_buf<E: serde::de::Error>(self, v: Vec<u8>) -> Result<Self::Value, E> {
                Ok(RbString { inner: v })
            }

            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(RbString {
                    inner: v.as_bytes().to_vec(),
                })
            }

            fn visit_borrowed_str<E: serde::de::Error>(
                self,
                v: &'de str,
            ) -> Result<Self::Value, E> {
                Ok(RbString {
                    inner: v.as_bytes().to_vec(),
                })
            }

            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<Self::Value, E> {
                Ok(RbString {
                    inner: v.into_bytes(),
                })
            }
        }

        deserializer.deserialize_byte_buf(RbStringVisitor)
    }
}

#[derive(Debug, Clone, thiserror::Error)]
#[error("{kind}")]
pub struct ParseRbStrError {
    kind: ParseRbStrErrorKind,
}

#[derive(Debug, Clone, thiserror::Error)]
enum ParseRbStrErrorKind {
    #[error("declared string len could not be converted to usize")]
    Len(#[from] TryFromIntError),
}

macro_rules! tri_opt {
    ($expr:expr) => {
        match $expr {
            Some(Ok(val)) => val,
            None => return None,
            Some(Err(e)) => {
                return Some(Err(ParseRbStrError {
                    kind: ParseRbStrErrorKind::from(e),
                }))
            }
        }
    };
}

impl<'a> TryFromCursor<'a> for &'a RbStr {
    type Error = ParseRbStrError;

    fn try_from_cursor(cursor: &mut Cursor<'a>) -> Option<Result<&'a RbStr, Self::Error>> {
        let len = tri_opt!(cursor.try_take::<FixNumLen>()).inner();
        cursor.take_n(len).map(RbStr::from_slice).map(Ok)
    }
}

// miri says these tests are fine
#[cfg(test)]
mod test {
    use super::RbStr;

    #[test]
    fn from_bytes() {
        let bytes: &[u8] = &[3, 4, 5, 6];
        let rb: &RbStr = bytes.into();
        assert_eq!(&rb.inner, bytes);
    }

    #[test]
    fn from_str() {
        let s = "hello world";
        let rb: &RbStr = s.into();
        assert_eq!(&rb.inner, s.as_bytes());
    }

    #[test]
    fn ordering() {
        let a: &RbStr = b"abc".as_slice().into();
        let b: &RbStr = b"abd".as_slice().into();
        assert!(a < b);
    }

    #[test]
    fn equality() {
        let from_str: &RbStr = "hello".into();
        let from_bytes: &RbStr = b"hello".as_slice().into();
        assert_eq!(from_str, from_bytes);
    }

    #[test]
    fn roundtrip() {
        let original = b"\x00\xff\x80";
        let rb: &RbStr = original.as_slice().into();
        assert_eq!(&rb.inner, original);
    }
}
