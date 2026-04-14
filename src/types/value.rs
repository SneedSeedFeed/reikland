use std::sync::Arc;

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
#[derive(Debug, Clone, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
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
    ClassOrModule(&'a RbStr),
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

impl<'a> MarshalValue<'a> {
    pub fn as_snake_case(&self) -> &'static str {
        self.into()
    }
}

// todo: wire this up
#[allow(dead_code)]
#[derive(Debug, Clone)]
enum OwnedMarshalValue {
    Nil,
    True,
    False,
    Fixnum(i32),
    Float(f64),
    Bignum(BigInt),
    SymbolLink(SymbolIdx),
    Symbol(Arc<[u8]>),
    String(Arc<[u8]>),
    Regex {
        pattern: Arc<[u8]>,
        flags: u8,
    },
    Array(Vec<ObjectIdx>),
    Hash(Vec<(Arc<OwnedMarshalValue>, Arc<OwnedMarshalValue>)>),
    HashDefault {
        pairs: Vec<(Arc<OwnedMarshalValue>, Arc<OwnedMarshalValue>)>,
        default: Arc<OwnedMarshalValue>,
    },
    ObjectRef(ObjectRefIdx),
    Object {
        class: SymbolIdx,
        ivars: Vec<(SymbolIdx, Arc<OwnedMarshalValue>)>,
    },
    Struct {
        name: SymbolIdx,
        members: Vec<(SymbolIdx, Arc<OwnedMarshalValue>)>,
    },
    Instance {
        inner: Arc<OwnedMarshalValue>,
        ivars: Vec<(SymbolIdx, Arc<OwnedMarshalValue>)>,
    },
    Extended {
        module: SymbolIdx,
        inner: Arc<OwnedMarshalValue>,
    },
    Class(Arc<[u8]>),
    Module(Arc<[u8]>),
    ClassOrModule(Arc<[u8]>),
    UserDefined {
        class: SymbolIdx,
        data: Vec<u8>,
    },
    UserMarshal {
        class: SymbolIdx,
        inner: Arc<OwnedMarshalValue>,
    },
    UserString {
        class: SymbolIdx,
        inner: Arc<OwnedMarshalValue>,
    },
    Data {
        class: SymbolIdx,
        inner: Arc<OwnedMarshalValue>,
    },
}
