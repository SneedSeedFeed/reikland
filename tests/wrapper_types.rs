// as clear by the horrid banner comments, ai generated tests

use std::collections::HashMap;

use serde::Deserialize;

use reikland::deserializer;
use reikland::deserializer_types::dual_key_map::{
    DualKeyMap, DualKeyMapInt, DualKeyVec, DualKeyVecSparse, DualKeyVecSparseHoley,
};
use reikland::types::encoding::RubyEncoding;
use reikland::{
    Encoding, Ignored, Ivar, RbHashDefault, RbObject, RbRegex, RbString, RbStruct, Transparent,
    TransparentOpt, WithEncoding,
};

const MARSHAL_DATA: &[u8] = include_bytes!("../test_data/wrapper_types.marshal");

fn parse() -> reikland::marshal::MarshalData<'static> {
    reikland::marshal::parse(MARSHAL_DATA).expect("failed to parse wrapper_types.marshal")
}

// ── Transparent ─────────────────────────────────────────────────────

#[test]
fn transparent_passes_through_bare_int() {
    #[derive(Deserialize)]
    struct Root {
        bare_int: Transparent<i32>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(*root.bare_int, 42);
}

#[test]
fn transparent_passes_through_bare_symbol() {
    #[derive(Deserialize)]
    struct Root {
        bare_symbol: Transparent<String>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(*root.bare_symbol, "my_symbol");
}

#[test]
fn transparent_unwraps_ivar_string() {
    // A UTF-8 string in marshal is Instance { String, {E: true} }.
    // Transparent should dig through the sequence to get the string.
    #[derive(Deserialize)]
    struct Root {
        utf8_string: Transparent<String>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(*root.utf8_string, "hello world");
}

// ── TransparentOpt ──────────────────────────────────────────────────

#[test]
fn transparent_opt_bare_value_gives_none() {
    #[derive(Deserialize)]
    struct Root {
        bare_int: TransparentOpt<i32, ()>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(*root.bare_int, 42);
    assert!(root.bare_int.1.is_none());
}

#[test]
fn transparent_opt_ivar_captures_second_element() {
    // Instance wrapper is a 2-element sequence: (value, ivar_map).
    // TransparentOpt should capture the ivar map as the second element.
    #[derive(Deserialize)]
    struct Root {
        utf8_string: TransparentOpt<String, Encoding>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(*root.utf8_string, "hello world");
    let enc = root
        .utf8_string
        .1
        .as_ref()
        .expect("should capture encoding");
    assert_eq!(enc.0, RubyEncoding::Utf8);
}

// ── Ivar ────────────────────────────────────────────────────────────

#[test]
fn ivar_discards_ivars_with_unit() {
    #[derive(Deserialize)]
    struct Root {
        utf8_string: Ivar<String>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(*root.utf8_string, "hello world");
}

#[test]
fn ivar_captures_ivars_as_hashmap() {
    #[derive(Deserialize)]
    struct Root {
        utf8_string: Ivar<String, HashMap<String, bool>>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(*root.utf8_string, "hello world");
    // UTF-8 strings have E: true
    assert!(root.utf8_string.ivars["E"]);
}

// ── WithEncoding / Encoding ─────────────────────────────────────────

#[test]
fn with_encoding_utf8() {
    #[derive(Deserialize)]
    struct Root {
        utf8_string: WithEncoding<String>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(*root.utf8_string, "hello world");
    assert_eq!(root.utf8_string.ivars.0, RubyEncoding::Utf8);
}

#[test]
fn with_encoding_ascii() {
    #[derive(Deserialize)]
    struct Root {
        ascii_string: WithEncoding<String>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(*root.ascii_string, "hello");
    assert_eq!(root.ascii_string.ivars.0, RubyEncoding::UsAscii);
}

#[test]
fn with_encoding_shift_jis() {
    // The sjis bytes are not valid UTF-8, so we can't use String for the inner value.
    // RbString captures the raw bytes via deserialize_byte_buf, unlike Vec<u8> which
    // would call deserialize_seq and fail.
    #[derive(Deserialize)]
    struct Root {
        sjis_string: WithEncoding<RbString>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(root.sjis_string.ivars.0, RubyEncoding::ShiftJis);
    // "こんにちは" in Shift_JIS
    assert_eq!(
        root.sjis_string.as_slice(),
        b"\x82\xb1\x82\xf1\x82\xc9\x82\xbf\x82\xcd"
    );
}

#[test]
fn encoding_deref() {
    #[derive(Deserialize)]
    struct Root {
        utf8_string: WithEncoding<String>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    // Encoding derefs to RubyEncoding
    let enc: &RubyEncoding = &root.utf8_string.ivars;
    assert_eq!(*enc, RubyEncoding::Utf8);
}

// ── RbObject ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct AnimalFields {
    // @name is an Instance-wrapped string in marshal, so Transparent unwraps it
    #[serde(rename = "@name")]
    name: Transparent<String>,
    #[serde(rename = "@legs")]
    legs: i32,
}

#[test]
fn rb_object_discards_class() {
    #[derive(Deserialize)]
    struct Root {
        animal: RbObject<AnimalFields>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(*root.animal.name, "cat");
    assert_eq!(root.animal.legs, 4);
}

#[test]
fn rb_object_captures_class() {
    #[derive(Deserialize)]
    struct Root {
        animal: RbObject<AnimalFields, String>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(root.animal.class, "Animal");
    assert_eq!(*root.animal.name, "cat");
    assert_eq!(root.animal.legs, 4);
}

#[test]
fn rb_object_deref_to_fields() {
    #[derive(Deserialize)]
    struct Root {
        animal: RbObject<AnimalFields>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    // Deref gives direct access to fields
    let name: &str = &root.animal.name;
    assert_eq!(name, "cat");
}

// ── RbStruct ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct PairFields {
    left: i32,
    right: i32,
}

#[test]
fn rb_struct_discards_name() {
    #[derive(Deserialize)]
    struct Root {
        pair: RbStruct<PairFields>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(root.pair.left, 100);
    assert_eq!(root.pair.right, 200);
}

#[test]
fn rb_struct_captures_name() {
    #[derive(Deserialize)]
    struct Root {
        pair: RbStruct<PairFields, String>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(root.pair.class, "Pair");
    assert_eq!(root.pair.left, 100);
    assert_eq!(root.pair.right, 200);
}

// ── RbRegex ─────────────────────────────────────────────────────────

#[test]
fn rb_regex_plain() {
    // Regex in marshal is Instance { Regex(pattern, flags), encoding_ivars }.
    // Ivar<RbRegex> captures the Instance as (inner=RbRegex, ivars=()).
    #[derive(Deserialize)]
    struct Root {
        regex_plain: Ivar<RbRegex>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(root.regex_plain.pattern, "hello");
    assert_eq!(root.regex_plain.flags, 0);
}

#[test]
fn rb_regex_with_flags() {
    #[derive(Deserialize)]
    struct Root {
        regex_flags: Ivar<RbRegex>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(root.regex_flags.pattern, "world");
    // Ruby: IGNORECASE=1, EXTENDED=2, MULTILINE=4
    assert_eq!(root.regex_flags.flags, 1 | 2 | 4);
}

#[test]
fn rb_regex_transparent_unwrap() {
    // Transparent<RbRegex> also works: unwraps the Instance, then RbRegex
    // handles the inner (pattern, flags) sequence.
    #[derive(Deserialize)]
    struct Root {
        regex_plain: Transparent<RbRegex>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(root.regex_plain.pattern, "hello");
    assert_eq!(root.regex_plain.flags, 0);
}

// ── RbHashDefault ───────────────────────────────────────────────────

#[test]
fn rb_hash_default_captures_default() {
    #[derive(Deserialize)]
    struct Root {
        hash_default: RbHashDefault<HashMap<String, i32>, i32>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(root.hash_default.default, 99);
    assert_eq!(root.hash_default.hash["x"], 10);
    assert_eq!(root.hash_default.hash["y"], 20);
    assert_eq!(root.hash_default.hash["z"], 30);
    assert_eq!(root.hash_default.hash.len(), 3);
}

#[test]
fn rb_hash_default_discards_default() {
    // With Ignored as the default (D=Ignored), the default value is deserialized
    // and discarded regardless of its type. Previously this required D=() which
    // only worked when the default was nil.
    #[derive(Deserialize)]
    struct Root {
        hash_default: RbHashDefault<HashMap<String, i32>>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(root.hash_default.default, Ignored);
    assert_eq!(root.hash_default.hash["x"], 10);
    assert_eq!(root.hash_default.hash["y"], 20);
    assert_eq!(root.hash_default.hash["z"], 30);
}

#[test]
fn rb_hash_default_deref_to_hash() {
    #[derive(Deserialize)]
    struct Root {
        hash_default: RbHashDefault<HashMap<String, i32>, i32>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    // Deref goes straight to the HashMap
    assert_eq!(root.hash_default.len(), 3);
    assert!(root.hash_default.contains_key("z"));
}

// ── DualKeyMap (keeps string keys, discards int keys) ───────────────

#[test]
fn dual_key_map_keeps_string_keys() {
    #[derive(Deserialize)]
    struct Root {
        mixed_hash: DualKeyMap<String, Transparent<String>>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(root.mixed_hash.0.len(), 3);
    assert_eq!(*root.mixed_hash.0["alpha"], "a");
    assert_eq!(*root.mixed_hash.0["beta"], "b");
    assert_eq!(*root.mixed_hash.0["gamma"], "c");
}

// ── DualKeyMapInt (keeps int keys, discards string keys) ────────────

#[test]
fn dual_key_map_int_keeps_int_keys() {
    #[derive(Deserialize)]
    struct Root {
        int_keyed_hash: DualKeyMapInt<i32, Transparent<String>>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(root.int_keyed_hash.0.len(), 3);
    assert_eq!(*root.int_keyed_hash.0[&10], "ten");
    assert_eq!(*root.int_keyed_hash.0[&20], "twenty");
    assert_eq!(*root.int_keyed_hash.0[&30], "thirty");
}

// ── DualKeyVec (int-keyed values in insertion order) ────────────────

#[test]
fn dual_key_vec_collects_in_order() {
    #[derive(Deserialize)]
    struct Root {
        mixed_hash: DualKeyVec<Transparent<String>>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(root.mixed_hash.0.len(), 3);
    assert_eq!(*root.mixed_hash.0[0], "zero");
    assert_eq!(*root.mixed_hash.0[1], "one");
    assert_eq!(*root.mixed_hash.0[2], "two");
}

// ── DualKeyVecSparse (int-keyed values indexed by key, no holes) ────

#[test]
fn dual_key_vec_sparse_indexes_by_key() {
    #[derive(Deserialize)]
    struct Root {
        mixed_hash: DualKeyVecSparse<Transparent<String>>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    // Keys are 0, 1, 2 -- contiguous, so this should work
    assert_eq!(root.mixed_hash.0.len(), 3);
    assert_eq!(*root.mixed_hash.0[0], "zero");
    assert_eq!(*root.mixed_hash.0[1], "one");
    assert_eq!(*root.mixed_hash.0[2], "two");
}

// ── DualKeyVecSparseHoley (int-keyed values with gaps allowed) ──────

#[test]
fn dual_key_vec_sparse_holey_allows_gaps() {
    #[derive(Deserialize)]
    struct Root {
        sparse_hash: DualKeyVecSparseHoley<Transparent<String>>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    let v = &root.sparse_hash.0;
    // Keys are 0, 5, 2 -- so vec length is 6 (indices 0..=5)
    assert_eq!(v.len(), 6);
    assert_eq!(v[0].as_ref().unwrap().0, "first");
    assert!(v[1].is_none());
    assert_eq!(v[2].as_ref().unwrap().0, "third");
    assert!(v[3].is_none());
    assert!(v[4].is_none());
    assert_eq!(v[5].as_ref().unwrap().0, "sixth");
}

#[test]
fn dual_key_vec_sparse_holey_iter_filled() {
    #[derive(Deserialize)]
    struct Root {
        sparse_hash: DualKeyVecSparseHoley<Transparent<String>>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    let filled: Vec<&str> = root
        .sparse_hash
        .iter_filled()
        .map(|t| t.0.as_str())
        .collect();
    assert_eq!(filled, vec!["first", "third", "sixth"]);
}

// ── DualKeyVecSparse rejects holes ──────────────────────────────────

#[test]
fn dual_key_vec_sparse_rejects_holes() {
    #[derive(Deserialize)]
    #[allow(dead_code)]
    struct Root {
        sparse_hash: DualKeyVecSparse<Transparent<String>>,
    }

    let data = parse();
    let result: Result<Root, _> = deserializer::from_marshal_data(&data);
    assert!(
        result.is_err(),
        "DualKeyVecSparse should reject sparse data with holes"
    );
}

// ── Combining wrappers ──────────────────────────────────────────────

#[test]
fn ivar_rb_regex_with_encoding() {
    // The full structure of a regex in marshal is:
    // Instance { Regex(pattern, flags), {E: true/false} }
    // So WithEncoding<RbRegex> captures both the regex and its encoding.
    #[derive(Deserialize)]
    struct Root {
        regex_plain: WithEncoding<RbRegex>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(root.regex_plain.inner.pattern, "hello");
    assert_eq!(root.regex_plain.inner.flags, 0);
    assert_eq!(root.regex_plain.ivars.0, RubyEncoding::UsAscii);
}

#[test]
fn transparent_opt_regex_captures_encoding() {
    // TransparentOpt<RbRegex, Encoding> unwraps Instance and captures encoding
    #[derive(Deserialize)]
    struct Root {
        regex_flags: TransparentOpt<RbRegex, Encoding>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(root.regex_flags.pattern, "world");
    assert_eq!(root.regex_flags.flags, 7);
    let enc = root.regex_flags.1.as_ref().expect("should have encoding");
    assert_eq!(enc.0, RubyEncoding::UsAscii);
}

#[test]
fn rb_object_with_transparent_fields() {
    // An Animal object has ivar-wrapped string fields.
    // Using Transparent lets us cut through to the string value.
    #[derive(Deserialize)]
    struct Root {
        animal: RbObject<AnimalFields, String>,
    }

    let data = parse();
    let root: Root = deserializer::from_marshal_data(&data).unwrap();
    assert_eq!(root.animal.class, "Animal");
    assert_eq!(*root.animal.name, "cat");
    assert_eq!(root.animal.legs, 4);
}
