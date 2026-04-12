// todo: better name than `Cursor` maybe? since im shadowing std::io::CUrsor
/// Cursor over bytes in Ruby marshal format
pub struct Cursor<'a> {
    slice: &'a [u8],
}

impl<'a> Cursor<'a> {
    pub const fn take_const<const N: usize>(&mut self) -> Option<&[u8; N]> {
        let slice = self.slice.first_chunk::<N>();

        if slice.is_some() {
            self.slice = self.slice.split_at(N).1;
        }

        slice
    }

    pub const fn take_n(&mut self, n: usize) -> Option<&[u8]> {
        if self.slice.len() < n {
            None
        } else {
            let (split, rem) = self.slice.split_at(n);
            self.slice = rem;
            Some(split)
        }
    }

    pub const fn take_1(&mut self) -> Option<u8> {
        let val = self.slice.first().copied();
        if val.is_some() {
            self.slice = self.slice.split_at(1).1;
        };
        val
    }

    pub fn take<T>(&mut self) -> Option<T>
    where
        T: FromCursor,
    {
        T::from_cursor(self)
    }

    pub fn try_take<T>(&mut self) -> Option<Result<T, <T as TryFromCursor>::Error>>
    where
        T: TryFromCursor,
    {
        T::try_from_cursor(self)
    }

    pub const fn len(&self) -> usize {
        self.slice.len()
    }

    pub const fn is_empty(&self) -> bool {
        self.slice.is_empty()
    }
}

/// Indicates a type may be constructed from [`Cursor`], where the only error would be not enough bytes (encoded as [None])
pub trait FromCursor: Sized + TryFromCursor {
    fn from_cursor(cursor: &mut Cursor<'_>) -> Option<Self>;
}

impl<T> FromCursor for T
where
    T: TryFromCursor<Error = std::convert::Infallible>,
{
    fn from_cursor(cursor: &mut Cursor<'_>) -> Option<Self> {
        T::try_from_cursor(cursor).map(Result::unwrap)
    }
}

// iirc marshal is only storing stuff in le so we can get away with just using from_le_bytes
macro_rules! cursor_numeric_impls {
    ([$($ty:ty),*]) => {
        $(
            impl TryFromCursor for $ty {
                type Error = std::convert::Infallible;

                fn try_from_cursor(cursor: &mut Cursor<'_>) -> Option<Result<$ty, std::convert::Infallible>> {
                    const SIZE: usize = std::mem::size_of::<$ty>();
                    cursor.take_const::<SIZE>().copied().map(<$ty>::from_le_bytes).map(Ok)
                }
            }
        )*
    };
}

cursor_numeric_impls!([u16, u32, u64, u128, i16, i32, i64, i128]);

impl TryFromCursor for u8 {
    type Error = std::convert::Infallible;

    fn try_from_cursor(cursor: &mut Cursor<'_>) -> Option<Result<Self, Self::Error>> {
        let (byte, rem) = cursor.slice.split_first()?;
        cursor.slice = rem;
        Some(Ok(*byte))
    }
}

impl TryFromCursor for i8 {
    type Error = std::convert::Infallible;

    fn try_from_cursor(cursor: &mut Cursor<'_>) -> Option<Result<Self, Self::Error>> {
        <u8 as FromCursor>::from_cursor(cursor)
            .map(u8::cast_signed)
            .map(Ok)
    }
}

/// Indicates a type may be fallibly constructed from a [`Cursor`]. Like [`FromCursor`], [`None`] indicates the cursor ran out of bytes
pub trait TryFromCursor: Sized {
    type Error: std::error::Error;

    fn try_from_cursor(cursor: &mut Cursor<'_>) -> Option<Result<Self, Self::Error>>;
}
