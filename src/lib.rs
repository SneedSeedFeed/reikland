//! Reikland parses and deserializes Ruby's [Marshal format](https://docs.ruby-lang.org/en/3.4/marshal_rdoc.html) with `serde` compatibility.
//! If you don't need `serde` compatibility you may prefer [alox-48](https://crates.io/crates/alox-48), which inspired this crate.
//!
//! # Quick start
//!
//! ```no_run
//! use serde::Deserialize;
//! use reikland::{from_bytes, Transparent};
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
//! let player: Transparent<Player> = from_bytes(data).unwrap(); // details on that `Transparent` type below
//! println!("{}", player.name);
//! ```
//!
//! # Wrapper types
//!
//! I found the marshal format to have a good degree of desync between the "intended" and "literal" ways to deserialize a value in Rust. For example: An instance variable is basically just a `(T, HashMap<Symbol, Value>)` but in many cases you (the lovely person reading this) just want `T`. However if I just made instance variables deserialize to `T` we are losing information so I made the executive decision to provide a collection of helpful wrappers to get at `T` with less pain.
//!
//! - [`Transparent<T>`] - deserializes `T`, unwrapping instance variable / sequence layers automatically.
//! - [`TransparentOpt<T, O>`] - like `Transparent` but also captures the second element of a sequence if present.
//! - [`Ivar<T, O>`] - deserializes an instance variable as its inner value plus the ivar map.
//! - [`RbObject<T, N>`] / [`RbStruct`] - deserializes a Ruby Object or Struct as its class name and field map.
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
//!
//! # Using the parser directly
//!
//! If the serde implementation doesn't work for you, the raw parser is exposed via [`marshal::parse`].
//! It produces a flat [`marshal::MarshalData`] with no recursive types, which the serde implementation also uses internally.

pub mod cursor;
pub mod deserializer;
pub mod deserializer_types;
pub mod marshal;
pub mod types;
pub mod version_number;

pub use deserializer::{from_bytes, from_marshal_data};
pub use deserializer_types::{
    Encoding, Ignored, Ivar, MixedKey, MixedKeyRef, RbHashDefault, RbObject, RbRegex, RbStruct,
    Transparent, TransparentOpt, WithEncoding,
    dual_key_map::{DualKeyMap, DualKeyVec},
};
pub use types::{
    encoding::RubyEncoding,
    regex::RbRegexStr,
    string::{RbStr, RbString},
    value::MarshalValue,
};
