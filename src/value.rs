//! A lossless, schema-free tree of Ruby marshal values that can be read *and* written.
//!
//! ```
//! use reikland::Value;
//!
//! let bytes = Value::array([Value::string("hi"), Value::symbol("hi"), 3.into()]).to_bytes();
//! let value = Value::from_bytes(&bytes).unwrap();
//! assert_eq!(value.to_bytes(), bytes);
//! ```
//!
//! Limitations:
//! - Cyclic object graphs (an object containing a link back to itself) can't be represented and fail to read.
//! - Symbols must be UTF-8.
//! - A link to a float that isn't a Ruby flonum, or to a bignum, is read as a copy of the number.
//!   Ruby treats these as equal values, but the output has the number written out again where the input had a link.
use std::sync::Arc;

use num_bigint::BigInt;

use crate::deserializer::MarshalDeserializeError;

mod read;
mod write;

pub(crate) const MAX_DEPTH: usize = 128;

/// A Ruby marshal value. See the [module docs](self).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Nil,
    Bool(bool),
    /// A fixnum, or a bignum that fits in an `i64`. The writer chooses the encoding.
    Int(i64),
    Float(f64),
    Symbol(Symbol),
    /// Any heap object. Shared [`Arc`]s are written as links.
    Ref(Arc<Object>),
}

/// An interned Ruby symbol, e.g. `:POKEBALL` or the `:@name` of an instance variable.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Symbol(Arc<str>);

/// A heap object: its contents plus the wrappers marshal can put around it.
#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub body: Body,
    /// Instance variables, including a string's encoding (`:E`). For a [`Body::Object`] these are its fields.
    pub ivars: Vec<(Symbol, Value)>,
    /// Modules the object was `extend`ed with (`e`), outermost first.
    pub extended: Vec<Symbol>,
    /// Subclass of a builtin, e.g. `class Name < String` (`C`).
    pub user_class: Option<Symbol>,
}

/// The contents of an [`Object`]
#[derive(Debug, Clone, PartialEq)]
pub enum Body {
    /// A plain Ruby object (`o`). Its fields are in [`Object::ivars`].
    Object {
        class: Symbol,
    },
    String(Vec<u8>),
    Regex {
        source: Vec<u8>,
        flags: u8,
    },
    Array(Vec<Value>),
    Hash {
        pairs: Vec<(Value, Value)>,
        default: Option<Value>,
    },
    Struct {
        class: Symbol,
        members: Vec<(Symbol, Value)>,
    },
    /// An integer too large for [`Value::Int`].
    BigInt(BigInt),
    Class(Vec<u8>),
    Module(Vec<u8>),
    /// The legacy `M` type.
    ClassOrModule(Vec<u8>),
    /// A class with `_dump`/`_load` (`u`), e.g. `Time`, or RGSS's `Table` and `Color`.
    UserDefined {
        class: Symbol,
        data: Vec<u8>,
    },
    /// A class with `marshal_dump`/`marshal_load` (`U`).
    UserMarshal {
        class: Symbol,
        data: Value,
    },
    Data {
        class: Symbol,
        data: Value,
    },
}

impl Value {
    /// Read a complete marshal stream, including the version header.
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, MarshalDeserializeError> {
        read::from_bytes(bytes)
    }

    /// Write a complete marshal stream, including the version header.
    ///
    /// # Panics
    /// If a string, array, or other collection has more than `i32::MAX` elements, which marshal
    /// can't encode.
    pub fn to_bytes(&self) -> Vec<u8> {
        write::to_bytes(self)
    }

    /// A UTF-8 string, tagged with its encoding the way Ruby tags string literals.
    pub fn string(s: &str) -> Self {
        Object {
            ivars: vec![(Symbol::from("E"), Value::Bool(true))],
            ..Object::new(Body::String(s.as_bytes().to_vec()))
        }
        .into()
    }

    pub fn symbol(name: &str) -> Self {
        Value::Symbol(name.into())
    }

    pub fn array(items: impl IntoIterator<Item = Value>) -> Self {
        Object::new(Body::Array(items.into_iter().collect())).into()
    }

    pub fn hash(pairs: impl IntoIterator<Item = (Value, Value)>) -> Self {
        Object::new(Body::Hash {
            pairs: pairs.into_iter().collect(),
            default: None,
        })
        .into()
    }

    /// An instance of `class` with the given instance variables, e.g.
    /// `Value::object("Pokemon::Move", [("@id", Value::symbol("TACKLE")), ("@pp", 35.into())])`.
    pub fn object<'a>(class: &str, ivars: impl IntoIterator<Item = (&'a str, Value)>) -> Self {
        Object {
            ivars: ivars.into_iter().map(|(k, v)| (k.into(), v)).collect(),
            ..Object::new(Body::Object {
                class: class.into(),
            })
        }
        .into()
    }

    pub fn is_nil(&self) -> bool {
        matches!(self, Value::Nil)
    }

    pub fn as_object(&self) -> Option<&Object> {
        match self {
            Value::Ref(object) => Some(object),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            Value::Int(n) => Some(*n),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Bool(b) => Some(*b),
            _ => None,
        }
    }

    pub fn as_symbol(&self) -> Option<&str> {
        match self {
            Value::Symbol(s) => Some(s.as_str()),
            _ => None,
        }
    }

    /// The contents of a string, if it is one and is valid UTF-8.
    pub fn as_str(&self) -> Option<&str> {
        match &self.as_object()?.body {
            Body::String(bytes) => std::str::from_utf8(bytes).ok(),
            _ => None,
        }
    }

    pub fn as_array(&self) -> Option<&[Value]> {
        match &self.as_object()?.body {
            Body::Array(items) => Some(items),
            _ => None,
        }
    }

    /// Instance variable lookup, e.g. `value.ivar("@name")`. `None` if this isn't an object or the
    /// ivar isn't set.
    pub fn ivar(&self, name: &str) -> Option<&Value> {
        self.as_object()?.ivar(name)
    }
}

impl Object {
    /// An object with no ivars or wrappers.
    pub fn new(body: Body) -> Self {
        Self {
            body,
            ivars: Vec::new(),
            extended: Vec::new(),
            user_class: None,
        }
    }

    pub fn ivar(&self, name: &str) -> Option<&Value> {
        self.ivars
            .iter()
            .find(|(k, _)| k.as_str() == name)
            .map(|(_, v)| v)
    }

    /// The class name for the variants that carry one.
    pub fn class(&self) -> Option<&Symbol> {
        match &self.body {
            Body::Object { class }
            | Body::Struct { class, .. }
            | Body::UserDefined { class, .. }
            | Body::UserMarshal { class, .. }
            | Body::Data { class, .. } => Some(class),
            _ => None,
        }
    }
}

impl Symbol {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for Symbol {
    fn from(name: &str) -> Self {
        Symbol(name.into())
    }
}

impl From<Symbol> for Value {
    fn from(symbol: Symbol) -> Self {
        Value::Symbol(symbol)
    }
}

impl From<Object> for Value {
    fn from(object: Object) -> Self {
        Value::Ref(Arc::new(object))
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<f64> for Value {
    fn from(f: f64) -> Self {
        Value::Float(f)
    }
}

macro_rules! int_conversions {
    ($($ty:ty),*) => {$(
        impl From<$ty> for Value {
            fn from(n: $ty) -> Self {
                Value::Int(n.into())
            }
        }
    )*};
}
int_conversions!(i8, i16, i32, i64, u8, u16, u32);

/// `None` is `nil`.
impl<T: Into<Value>> From<Option<T>> for Value {
    fn from(value: Option<T>) -> Self {
        value.map_or(Value::Nil, Into::into)
    }
}
