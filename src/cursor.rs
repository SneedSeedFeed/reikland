use crate::{
    cursor::{
        object_table::{ObjectIdx, ObjectTable},
        symbol_table::{SymbolIdx, SymbolTable},
    },
    types::{string::RbStr, value::MarshalValue},
};

pub mod object_table;
pub mod symbol_table;

// todo: better name than `Cursor` maybe? since im shadowing std::io::CUrsor
/// Cursor over bytes in Ruby marshal format
pub struct Cursor<'a> {
    slice: &'a [u8],
    symbols: SymbolTable<'a>,
    objects: ObjectTable<'a>,
}

impl<'a> Cursor<'a> {
    /// Create a new [`Cursor`]
    pub fn new(slice: &'a [u8]) -> Self {
        Self {
            slice,
            symbols: SymbolTable::new(),
            objects: ObjectTable::new(),
        }
    }

    /// Array returning equivalent of [`Self::take_n`]
    pub const fn take_const<const N: usize>(&mut self) -> Option<&'a [u8; N]> {
        let slice = self.slice.first_chunk::<N>();

        if slice.is_some() {
            self.slice = self.slice.split_at(N).1;
        }

        slice
    }

    /// Take n bytes from this cursor
    pub const fn take_n(&mut self, n: usize) -> Option<&'a [u8]> {
        if self.slice.len() < n {
            None
        } else {
            let (split, rem) = self.slice.split_at(n);
            self.slice = rem;
            Some(split)
        }
    }

    /// Take 1 byte from this cursor
    pub const fn take_1(&mut self) -> Option<u8> {
        let val = self.slice.first().copied();
        if val.is_some() {
            self.slice = self.slice.split_at(1).1;
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
        self.slice.len()
    }

    /// Is this cursor's byte slice empty
    pub const fn is_empty(&self) -> bool {
        self.slice.is_empty()
    }

    /// Push a symbol into the symbol table, returning its [`SymbolIdx`].
    pub fn push_symbol(&mut self, symbol: &'a RbStr) -> SymbolIdx {
        self.symbols.push(symbol)
    }

    /// Look up a symbol by index.
    pub fn get_symbol(&self, idx: SymbolIdx) -> Option<&'a RbStr> {
        self.symbols.get(idx)
    }

    /// Push a value into the object store, returning its [`ObjectIdx`].
    pub fn push_object(&mut self, value: MarshalValue<'a>) -> ObjectIdx {
        self.objects.push_object(value)
    }

    /// Register an object in the object reference table (for `@` links).
    pub fn push_object_ref(&mut self, idx: ObjectIdx) {
        self.objects.push_object_ref(idx);
    }

    /// Resolve a marshal `@` reference to an [`ObjectIdx`].
    pub fn get_object_ref(&self, ref_idx: usize) -> Option<ObjectIdx> {
        self.objects.get_by_ref(ref_idx)
    }

    /// Consume the cursor, returning the accumulated tables.
    pub fn into_tables(self) -> (SymbolTable<'a>, ObjectTable<'a>) {
        (self.symbols, self.objects)
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
        let (byte, rem) = cursor.slice.split_first()?;
        cursor.slice = rem;
        Some(Ok(*byte))
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
