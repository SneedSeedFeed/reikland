//! This module has a collection of types you may run into with your marshal data, with some reasonable Deserialize and Serialize implementations for them.

pub mod dual_key_map;

pub mod mixed_key;
pub use mixed_key::{MixedKey, MixedKeyRef};
