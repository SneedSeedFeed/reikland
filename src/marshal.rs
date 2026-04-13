use crate::{
    cursor::{
        object_table::{ObjectIdx, ObjectTable},
        symbol_table::{SymbolIdx, SymbolTable},
    },
    types::{string::RbStr, value::MarshalValue},
    version_number::VersionNumber,
};

/// The fully parsed output of a Ruby marshal byte stream.
///
/// Symbols and objects are stored in flat tables, referenced by [`SymbolIdx`] and [`ObjectIdx`] respectively.
#[derive(Debug)]
pub struct MarshalData<'a> {
    pub version: VersionNumber,
    pub symbols: SymbolTable<'a>,
    pub objects: ObjectTable<'a>,
    pub root: ObjectIdx,
}

impl<'a> MarshalData<'a> {
    pub fn symbol(&self, idx: SymbolIdx) -> Option<&'a RbStr> {
        self.symbols.get(idx)
    }

    pub fn object(&self, idx: ObjectIdx) -> &MarshalValue<'a> {
        &self.objects[idx]
    }

    pub fn root(&self) -> &MarshalValue<'a> {
        self.object(self.root)
    }
}
