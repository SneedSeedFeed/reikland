pub mod bignum;
pub mod encoding;
pub mod fixnum;
pub mod float;
pub mod hash_default;
pub mod ivar;
pub mod rb_object;
pub mod regex;
pub mod string;
pub mod transparent;
pub mod type_byte;
pub mod value;

// todo: a lot of these are basically just wrappers over deserialization for (T, O) with different "expecting" strings. Could DRY up?
pub use {
    encoding::RubyEncoding,
    hash_default::RbHashDefault,
    ivar::{Encoding, Ivar, WithEncoding},
    rb_object::{RbObject, RbStruct},
    regex::{RbRegex, RbRegexStr},
    transparent::Transparent,
    value::MarshalValue,
};
