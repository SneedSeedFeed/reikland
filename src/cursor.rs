// todo: better name than `Cursor` maybe? since im shadowing std::io::CUrsor
/// Cursor over bytes in Ruby marshal format.
/// Tracks its absolute position so values can be revisited when resolving object references (`@`).
pub struct Cursor<'a> {
    input: &'a [u8],
    pos: usize,
}

impl<'a> Cursor<'a> {
    /// Create a new [`Cursor`]
    pub fn new(input: &'a [u8]) -> Self {
        Self { input, pos: 0 }
    }

    /// The absolute byte offset of the next unread byte
    pub const fn pos(&self) -> usize {
        self.pos
    }

    /// Jump to an absolute byte offset
    pub const fn set_pos(&mut self, pos: usize) {
        self.pos = pos;
    }

    /// Look at the next byte without consuming it
    pub const fn peek(&self) -> Option<u8> {
        if self.pos < self.input.len() {
            Some(self.input[self.pos])
        } else {
            None
        }
    }

    /// The bytes that have not been consumed yet
    const fn remaining(&self) -> &'a [u8] {
        self.input.split_at(self.pos).1
    }

    /// Array returning equivalent of [`Self::take_n`]
    pub const fn take_const<const N: usize>(&mut self) -> Option<&'a [u8; N]> {
        let slice = self.remaining().first_chunk::<N>();

        if slice.is_some() {
            self.pos += N;
        }

        slice
    }

    /// Take n bytes from this cursor
    pub const fn take_n(&mut self, n: usize) -> Option<&'a [u8]> {
        let remaining = self.remaining();
        if remaining.len() < n {
            None
        } else {
            self.pos += n;
            Some(remaining.split_at(n).0)
        }
    }

    /// Take 1 byte from this cursor
    pub const fn take_1(&mut self) -> Option<u8> {
        let val = self.peek();
        if val.is_some() {
            self.pos += 1;
        };
        val
    }

    /// Try to take an infallible value from this cursor. Returns [`None`] if not enough bytes remained to construct `T`
    pub fn take<T>(&mut self) -> Option<T>
    where
        T: FromCursor<'a>,
    {
        T::from_cursor(self)
    }

    /// Try to take a fallible value from this cursor. Returns [`None`] if not enough bytes remained to construct `T`
    pub fn try_take<T>(&mut self) -> Option<Result<T, <T as TryFromCursor<'a>>::Error>>
    where
        T: TryFromCursor<'a>,
    {
        T::try_from_cursor(self)
    }

    /// How many bytes remain in this cursor's slice
    pub const fn len(&self) -> usize {
        self.remaining().len()
    }

    /// Is this cursor's byte slice empty
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Indicates a type may be constructed from [`Cursor`], where the only error would be not enough bytes (encoded as [None])
pub trait FromCursor<'a>: Sized + TryFromCursor<'a> {
    fn from_cursor(cursor: &mut Cursor<'a>) -> Option<Self>;
}

impl<'a, T> FromCursor<'a> for T
where
    T: TryFromCursor<'a, Error = std::convert::Infallible>,
{
    fn from_cursor(cursor: &mut Cursor<'a>) -> Option<Self> {
        T::try_from_cursor(cursor).map(Result::unwrap)
    }
}

// iirc marshal is only storing stuff in le so we can get away with just using from_le_bytes
macro_rules! cursor_numeric_impls {
    ([$($ty:ty),*]) => {
        $(
            impl TryFromCursor<'_> for $ty {
                type Error = std::convert::Infallible;

                fn try_from_cursor(cursor: &mut Cursor<'_>) -> Option<Result<$ty, std::convert::Infallible>>
                {
                    const SIZE: usize = std::mem::size_of::<$ty>();
                    cursor.take_const::<SIZE>().copied().map(<$ty>::from_le_bytes).map(Ok)
                }
            }
        )*
    };
}

cursor_numeric_impls!([u16, u32, u64, u128, i16, i32, i64, i128]);

impl TryFromCursor<'_> for u8 {
    type Error = std::convert::Infallible;

    fn try_from_cursor<'a>(cursor: &mut Cursor<'a>) -> Option<Result<Self, Self::Error>> {
        cursor.take_1().map(Ok)
    }
}

impl TryFromCursor<'_> for i8 {
    type Error = std::convert::Infallible;

    fn try_from_cursor(cursor: &mut Cursor<'_>) -> Option<Result<Self, Self::Error>> {
        <u8 as FromCursor>::from_cursor(cursor)
            .map(u8::cast_signed)
            .map(Ok)
    }
}

/// Indicates a type may be fallibly constructed from a [`Cursor`]. [`None`] indicates the cursor ran out of bytes
pub trait TryFromCursor<'a>: Sized {
    type Error: std::error::Error;

    fn try_from_cursor(cursor: &mut Cursor<'a>) -> Option<Result<Self, Self::Error>>;
}
