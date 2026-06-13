// Disclaimer: These tests are AI generated
//
// test_data/all_types.marshal is a single root array of 61 values covering every marshal type.
// v0.1 walked the parse tree directly; since 0.2 parses in a single phase there is no tree, so
// the root is deserialized as one big tuple struct with the exact expected type in each slot.
use std::collections::HashMap;

use reikland::{Ignored, Ivar, RbObject, RbRegex, RbStruct, Transparent, from_bytes};
use serde::{Deserialize, de::IgnoredAny};

const MARSHAL_DATA: &[u8] = include_bytes!("../test_data/all_types.marshal");

/// Counts the entries of a map while discarding the contents, for slots where only the pair
/// count is known.
#[derive(Debug, PartialEq)]
struct CountMap(usize);

impl<'de> Deserialize<'de> for CountMap {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct CountMapVisitor;

        impl<'de> serde::de::Visitor<'de> for CountMapVisitor {
            type Value = CountMap;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a map")
            }

            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                mut map: A,
            ) -> Result<CountMap, A::Error> {
                let mut count = 0;
                while map.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {
                    count += 1;
                }
                Ok(CountMap(count))
            }
        }

        deserializer.deserialize_map(CountMapVisitor)
    }
}

#[derive(Deserialize)]
struct AllTypes<'a>(
    (),   // 0: nil
    bool, // 1: true
    bool, // 2: false
    // -- Fixnum edge cases (3-23) --
    i32, // 3: 0
    i32, // 4: 1
    i32, // 5: 122
    i32, // 6: -1
    i32, // 7: -123
    i32, // 8: 123 (1-byte positive)
    i32, // 9: 255
    i32, // 10: -124 (1-byte negative)
    i32, // 11: -256
    i32, // 12: 256 (2-byte positive)
    i32, // 13: 65535
    i32, // 14: -257 (2-byte negative)
    i32, // 15: -65536
    i32, // 16: 65536 (3-byte positive)
    i32, // 17: 16777215
    i32, // 18: -65537 (3-byte negative)
    i32, // 19: -16777216
    i32, // 20: 16777216 (4-byte)
    i32, // 21: 1073741823 (max fixnum)
    i32, // 22: -16777217
    i32, // 23: -1073741824 (min fixnum)
    // -- Float edge cases (24-33) --
    f64, // 24: 0.0
    f64, // 25: -0.0
    f64, // 26: 1.5
    f64, // 27: -1.5
    f64, // 28: inf
    f64, // 29: -inf
    f64, // 30: NaN
    f64, // 31: f64::MAX
    f64, // 32: f64::MIN_POSITIVE
    f64, // 33: smallest subnormal
    // -- Bignum edge cases (34-38) --
    i64,     // 34: 2^30
    i64,     // 35: -(2^30 + 1)
    i128,    // 36: 2^64
    i128,    // 37: -(2^64)
    Ignored, // 38: 2^128, too big for any integer type
    // -- Symbol and strings (39-43) --
    &'a str,              // 39: :test_symbol
    Transparent<&'a str>, // 40: "hello world" (Instance-wrapped)
    Transparent<&'a str>, // 41: "" (Instance-wrapped)
    &'a [u8],             // 42: binary string (ASCII-8BIT, not Instance-wrapped)
    Transparent<&'a str>, // 43: emoji (Instance-wrapped)
    // -- Regex (44-45, Instance-wrapped) --
    Ivar<RbRegex<&'a str>>, // 44: /simple/
    Ivar<RbRegex<&'a str>>, // 45: /with flags/imx
    // -- Arrays (46-47) --
    Vec<i32>,                                  // 46: []
    (i32, Transparent<&'a str>, &'a str, f64), // 47: [1, "two", :three, 4.0]
    // -- Hashes (48-50) --
    HashMap<&'a str, i32>,                  // 48: {}
    CountMap,                               // 49: hash with 3 pairs
    reikland::RbHashDefault<CountMap, i32>, // 50: Hash.new(42) with 2 pairs
    // -- Object / Struct / Extended (51-53) --
    RbObject<CountMap, &'a str>, // 51: TestObject with 2 ivars
    RbStruct<HashMap<&'a str, i32>, &'a str>, // 52: TestStruct(100, 200)
    (RbObject<CountMap, &'a str>, &'a str), // 53: TestObject extended with TestModule
    // -- Class / Module (54-55) --
    &'a str, // 54: String
    &'a str, // 55: Kernel
    // -- User serialization (56-58) --
    Ivar<(&'a [u8], &'a str)>, // 56: UserDefined (Instance-wrapped)
    (Vec<i32>, &'a str),       // 57: UserMarshal of [10, 20, 30]
    Ivar<(&'a str, &'a str)>,  // 58: UserString "subclassed string" (Instance-wrapped)
    // -- Object references (59-60) --
    Transparent<&'a str>, // 59: "shared" (Instance-wrapped)
    Transparent<&'a str>, // 60: object reference back to 59
);

#[test]
fn deserialize_all_types() {
    let all: AllTypes = from_bytes(MARSHAL_DATA).expect("failed to deserialize marshal data");

    // -- Nil, True, False --
    assert!(all.1);
    assert!(!all.2);

    // -- Fixnum edge cases --
    assert_eq!(all.3, 0);
    assert_eq!(all.4, 1);
    assert_eq!(all.5, 122);
    assert_eq!(all.6, -1);
    assert_eq!(all.7, -123);
    assert_eq!(all.8, 123);
    assert_eq!(all.9, 255);
    assert_eq!(all.10, -124);
    assert_eq!(all.11, -256);
    assert_eq!(all.12, 256);
    assert_eq!(all.13, 65535);
    assert_eq!(all.14, -257);
    assert_eq!(all.15, -65536);
    assert_eq!(all.16, 65536);
    assert_eq!(all.17, 16777215);
    assert_eq!(all.18, -65537);
    assert_eq!(all.19, -16777216);
    assert_eq!(all.20, 16777216);
    assert_eq!(all.21, 1073741823);
    assert_eq!(all.22, -16777217);
    assert_eq!(all.23, -1073741824);

    // -- Float edge cases (bit-exact where it matters) --
    assert_eq!(all.24.to_bits(), 0.0f64.to_bits());
    assert_eq!(all.25.to_bits(), (-0.0f64).to_bits());
    assert_eq!(all.26, 1.5);
    assert_eq!(all.27, -1.5);
    assert_eq!(all.28, f64::INFINITY);
    assert_eq!(all.29, f64::NEG_INFINITY);
    assert!(all.30.is_nan());
    assert_eq!(all.31, f64::MAX);
    assert_eq!(all.32, 2.2250738585072014e-308); // f64::MIN_POSITIVE
    assert_eq!(all.33, 5.0e-324); // smallest subnormal

    // -- Bignum edge cases --
    assert_eq!(all.34, 1 << 30);
    assert_eq!(all.35, -(1i64 << 30) - 1);
    assert_eq!(all.36, 1 << 64);
    assert_eq!(all.37, -(1i128 << 64));

    // -- Symbol and strings --
    assert_eq!(all.39, "test_symbol");
    assert_eq!(*all.40, "hello world");
    assert_eq!(*all.41, "");
    assert_eq!(all.42, b"binary\x00data");
    assert_eq!(*all.43, "\u{1F600}");

    // -- Regex --
    assert_eq!(all.44.pattern, "simple");
    assert_eq!(all.44.flags, 0);
    assert_eq!(all.45.pattern, "with flags");
    // Ruby regex flags: IGNORECASE=1, EXTENDED=2, MULTILINE=4
    assert_eq!(all.45.flags, 1 | 2 | 4);

    // -- Arrays --
    assert!(all.46.is_empty());
    let (one, two, three, four) = all.47;
    assert_eq!(one, 1);
    assert_eq!(*two, "two");
    assert_eq!(three, "three");
    assert_eq!(four, 4.0);

    // -- Hashes --
    assert!(all.48.is_empty());
    assert_eq!(all.49, CountMap(3));
    assert_eq!(all.50.hash, CountMap(2));
    assert_eq!(all.50.default, 42);

    // -- Object / Struct / Extended --
    assert_eq!(all.51.class, "TestObject");
    assert_eq!(all.51.fields, CountMap(2));
    assert_eq!(all.52.class, "TestStruct");
    let mut members: Vec<i32> = all.52.fields.values().copied().collect();
    members.sort_unstable();
    assert_eq!(members, vec![100, 200]);
    let (extended, module) = all.53;
    assert_eq!(module, "TestModule");
    assert_eq!(extended.class, "TestObject");
    assert_eq!(extended.fields, CountMap(2));

    // -- Class / Module --
    assert_eq!(all.54, "String");
    assert_eq!(all.55, "Kernel");

    // -- User serialization --
    let (payload, class) = all.56.inner;
    assert_eq!(class, "UserDefinedClass");
    assert_eq!(payload, b"custom_payload");
    let (values, class) = all.57;
    assert_eq!(class, "UserMarshalClass");
    assert_eq!(values, vec![10, 20, 30]);
    let (string, class) = all.58.inner;
    assert_eq!(class, "MyString");
    assert_eq!(string, "subclassed string");

    // -- Object references --
    // 60 is a `@` link back to the Instance-wrapped "shared" string at 59
    assert_eq!(*all.59, "shared");
    assert_eq!(*all.60, "shared");
}
