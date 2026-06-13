//! Reikland parses and deserializes Ruby's [Marshal format](https://docs.ruby-lang.org/en/3.4/marshal_rdoc.html) with `serde` compatibility.
//! If you don't need `serde` compatibility you may prefer [alox-48](https://crates.io/crates/alox-48), which inspired this crate.
//!
//! # Quick start
//!
//! ```no_run
//! use serde::Deserialize;
//! use reikland::{DeserializerConfig, from_bytes_with_config};
//!
//! #[derive(Deserialize)]
//! struct Player<'a> {
//!     #[serde(rename = "@name")]
//!     name: &'a str,
//!     #[serde(rename = "@level")]
//!     level: i32,
//! }
//!
//! let data: &[u8] = todo!("read a .rxdata / Marshal.dump file");
//! let player: Player = from_bytes_with_config(data, DeserializerConfig::opinionated()).unwrap();
//! println!("{}", player.name);
//! ```
//!
//! # Opinionated deserialization
//!
//! I found the marshal format to have a good degree of desync between the "intended" and "literal" ways to deserialize a value in Rust. For example: An instance variable is basically just a `(T, HashMap<Symbol, Value>)` but in many cases you (the lovely person reading this) just want `T`.
//! The [`DeserializerConfig`] passed to [`from_bytes_with_config`] decides which of those you get: with all flags off (the [`from_bytes`] default) nothing is lost and the wrapper types below can be used to get at the meat, while [`DeserializerConfig::opinionated`] flattens every wrapper into its useful contents so plain Rust types "just work", as in the example above.
//!
//! # Wrapper types
//!
//! When deserializing strictly, these wrappers get at the "intended" value with less pain.
//! Every marshal type that comes through as a sequence leads with its useful value (inner
//! value, fields, payload...) followed by the extra information (ivars, class name...), which
//! is what lets `Transparent` grab the right element:
//!
//! - [`Transparent<T>`] - deserializes `T`, unwrapping instance variable / sequence layers automatically.
//! - [`TransparentOpt<T, O>`] - like `Transparent` but also captures the second element of a sequence if present.
//! - [`Ivar<T, O>`] - deserializes an instance variable as its inner value plus the ivar map.
//! - [`RbObject<T, N>`] / [`RbStruct`] - deserializes a Ruby Object or Struct as its field map plus the class name.
//!
//! - [`RbStr`] / [`RbString`] - borrowed and owned byte strings.
//! - [`RbRegex<P>`] - deserializes a Ruby Regex as its pattern and flags byte.
//! - [`Encoding`] - extracts the Ruby encoding from an ivar.
//! - [`WithEncoding<T>`](deserializer_types::WithEncoding) is an alias for `Ivar<T, Encoding>`.
//!
//! - [`RbHashDefault<T, D>`] - deserializes a Hash-with-default as the hash and its default value.
//!
//! - [`MixedKey`] / [`MixedKeyRef`] - represent hash keys that can be either integer or string.
//! - [`DualKeyMap`] / [`DualKeyVec`] / etc... - the [`dual_key_map`][deserializer_types::dual_key_map] module exports various ways to access maps that have both integer and string keys
//!
//! All wrapper structs implement `Deref`/`DerefMut` to their primary field so they can be used without unwrapping in most cases. Or you can destructure them as the fields are public.

pub mod cursor;
pub mod deserializer;
pub mod deserializer_types;
pub mod types;
pub mod version_number;

pub use deserializer::{
    Deserializer, DeserializerConfig, MarshalDeserializeError, from_bytes, from_bytes_with_config,
};
pub use deserializer_types::{
    Encoding, Ignored, Ivar, MixedKey, MixedKeyRef, RbHashDefault, RbObject, RbRegex, RbStruct,
    Transparent, TransparentOpt, WithEncoding,
    dual_key_map::{DualKeyMap, DualKeyVec},
};
pub use types::{
    encoding::RubyEncoding,
    regex::RbRegexStr,
    string::{RbStr, RbString},
};
