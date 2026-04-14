//! This module has a collection of types you may run into with your marshal data, with some reasonable Deserialize and Serialize implementations for them.

pub mod dual_key_map;
pub mod hash_default;
pub mod ivar;
pub mod mixed_key;
pub mod rb_object;
pub mod regex;
pub mod transparent;

pub use {
    hash_default::RbHashDefault,
    ivar::{Encoding, Ivar, WithEncoding},
    mixed_key::{MixedKey, MixedKeyRef},
    rb_object::{RbObject, RbStruct},
    regex::RbRegex,
    transparent::Transparent,
};
