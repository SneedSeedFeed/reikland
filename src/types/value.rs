use num_bigint::BigInt;

use crate::cursor::{
    object_table::{ObjectIdx, ObjectRefIdx},
    symbol_table::SymbolIdx,
};

use super::string::RbStr;

/// A parsed Ruby marshal value.
///
/// Types reference children by [`ObjectIdx`] into the [`ObjectTable`][crate::cursor::object_table::ObjectTable].
/// Symbol references use [`SymbolIdx`] into the [`SymbolTable`][crate::cursor::symbol_table::SymbolTable].
#[derive(Debug, Clone)]
pub enum MarshalValue<'a> {
    Nil,
    True,
    False,
    Fixnum(i32),
    Float(f64),
    Bignum(BigInt),
    SymbolLink(SymbolIdx),
    Symbol(&'a RbStr),
    String(&'a RbStr),
    Regex {
        pattern: &'a RbStr,
        flags: u8,
    },
    Array(Vec<ObjectIdx>),
    Hash(Vec<(ObjectIdx, ObjectIdx)>),
    HashDefault {
        pairs: Vec<(ObjectIdx, ObjectIdx)>,
        default: ObjectIdx,
    },
    ObjectRef(ObjectRefIdx),
    Object {
        class: SymbolIdx,
        ivars: Vec<(SymbolIdx, ObjectIdx)>,
    },
    Struct {
        name: SymbolIdx,
        members: Vec<(SymbolIdx, ObjectIdx)>,
    },
    Instance {
        inner: ObjectIdx,
        ivars: Vec<(SymbolIdx, ObjectIdx)>,
    },
    Extended {
        module: SymbolIdx,
        inner: ObjectIdx,
    },
    Class(&'a RbStr),
    Module(&'a RbStr),
    UserDefined {
        class: SymbolIdx,
        data: &'a [u8],
    },
    UserMarshal {
        class: SymbolIdx,
        inner: ObjectIdx,
    },
    UserString {
        class: SymbolIdx,
        inner: ObjectIdx,
    },
    Data {
        class: SymbolIdx,
        inner: ObjectIdx,
    },
}
