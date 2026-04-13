// Disclaimer: These tests are AI generated
use num_bigint::BigInt;
use reikland::{
    cursor::object_table::ObjectIdx,
    marshal::{self, MarshalData},
    types::value::MarshalValue,
};

const MARSHAL_DATA: &[u8] = include_bytes!("../test_data/all_types.marshal");

/// Get element `i` from the root array.
fn obj<'a>(data: &'a MarshalData<'a>, elements: &[ObjectIdx], i: usize) -> &'a MarshalValue<'a> {
    data.object(elements[i])
}

/// Unwrap an Instance wrapper, returning the inner value. Panics if not an Instance.
fn unwrap_instance<'a>(
    data: &'a MarshalData<'a>,
    elements: &[ObjectIdx],
    i: usize,
) -> (
    &'a MarshalValue<'a>,
    &'a [(reikland::cursor::symbol_table::SymbolIdx, ObjectIdx)],
) {
    match obj(data, elements, i) {
        MarshalValue::Instance { inner, ivars } => (data.object(*inner), ivars.as_slice()),
        other => panic!("expected Instance at index {i}, got {other:?}"),
    }
}

fn assert_fixnum(data: &MarshalData, elements: &[ObjectIdx], i: usize, expected: i32) {
    match obj(data, elements, i) {
        MarshalValue::Fixnum(v) => assert_eq!(*v, expected, "fixnum at index {i}"),
        other => panic!("expected Fixnum({expected}) at index {i}, got {other:?}"),
    }
}

fn assert_float(data: &MarshalData, elements: &[ObjectIdx], i: usize, expected: f64) {
    match obj(data, elements, i) {
        MarshalValue::Float(v) => {
            if expected.is_nan() {
                assert!(v.is_nan(), "expected NaN at index {i}, got {v}");
            } else {
                assert_eq!(
                    v.to_bits(),
                    expected.to_bits(),
                    "float at index {i}: got {v}, expected {expected}"
                );
            }
        }
        other => panic!("expected Float at index {i}, got {other:?}"),
    }
}

fn assert_string(data: &MarshalData, elements: &[ObjectIdx], i: usize, expected: &[u8]) {
    let (inner, _ivars) = unwrap_instance(data, elements, i);
    match inner {
        MarshalValue::String(s) => {
            assert_eq!(s.as_slice(), expected, "string at index {i}");
        }
        other => panic!("expected String inside Instance at index {i}, got {other:?}"),
    }
}

#[test]
fn parse_all_types() {
    let data = marshal::parse(MARSHAL_DATA).expect("failed to parse marshal data");

    let MarshalValue::Array(elements) = data.root() else {
        panic!("root is not an Array: {:?}", data.root());
    };
    assert_eq!(elements.len(), 61, "root array length");

    // -- Nil, True, False (indices 0-2) --
    assert!(matches!(obj(&data, elements, 0), MarshalValue::Nil));
    assert!(matches!(obj(&data, elements, 1), MarshalValue::True));
    assert!(matches!(obj(&data, elements, 2), MarshalValue::False));

    // -- Fixnum edge cases (indices 3-23) --
    // Zero
    assert_fixnum(&data, elements, 3, 0);
    // Single-byte positive (1..122)
    assert_fixnum(&data, elements, 4, 1);
    assert_fixnum(&data, elements, 5, 122);
    // Single-byte negative (-123..-1)
    assert_fixnum(&data, elements, 6, -1);
    assert_fixnum(&data, elements, 7, -123);
    // 1-byte positive (0x01 prefix)
    assert_fixnum(&data, elements, 8, 123);
    assert_fixnum(&data, elements, 9, 255);
    // 1-byte negative (0xff prefix)
    assert_fixnum(&data, elements, 10, -124);
    assert_fixnum(&data, elements, 11, -256);
    // 2-byte positive (0x02 prefix)
    assert_fixnum(&data, elements, 12, 256);
    assert_fixnum(&data, elements, 13, 65535);
    // 2-byte negative (0xfe prefix)
    assert_fixnum(&data, elements, 14, -257);
    assert_fixnum(&data, elements, 15, -65536);
    // 3-byte positive (0x03 prefix)
    assert_fixnum(&data, elements, 16, 65536);
    assert_fixnum(&data, elements, 17, 16777215);
    // 3-byte negative (0xfd prefix)
    assert_fixnum(&data, elements, 18, -65537);
    assert_fixnum(&data, elements, 19, -16777216);
    // 4-byte (0x04/0xfc prefix)
    assert_fixnum(&data, elements, 20, 16777216);
    assert_fixnum(&data, elements, 21, 1073741823); // max fixnum
    assert_fixnum(&data, elements, 22, -16777217);
    assert_fixnum(&data, elements, 23, -1073741824); // min fixnum

    // -- Float edge cases (indices 24-33) --
    assert_float(&data, elements, 24, 0.0);
    assert_float(&data, elements, 25, -0.0);
    assert_float(&data, elements, 26, 1.5);
    assert_float(&data, elements, 27, -1.5);
    assert_float(&data, elements, 28, f64::INFINITY);
    assert_float(&data, elements, 29, f64::NEG_INFINITY);
    assert_float(&data, elements, 30, f64::NAN);
    assert_float(&data, elements, 31, f64::MAX);
    assert_float(&data, elements, 32, 2.2250738585072014e-308); // f64::MIN_POSITIVE
    assert_float(&data, elements, 33, 5.0e-324); // smallest subnormal

    // -- Bignum edge cases (indices 34-38) --
    let assert_bignum = |i: usize, expected: &str| match obj(&data, elements, i) {
        MarshalValue::Bignum(v) => {
            let expected: BigInt = expected.parse().unwrap();
            assert_eq!(*v, expected, "bignum at index {i}");
        }
        other => panic!("expected Bignum at index {i}, got {other:?}"),
    };
    assert_bignum(34, "1073741824"); // 2^30
    assert_bignum(35, "-1073741825"); // -(2^30 + 1)
    assert_bignum(36, "18446744073709551616"); // 2^64
    assert_bignum(37, "-18446744073709551616"); // -(2^64)
    assert_bignum(38, "340282366920938463463374607431768211456"); // 2^128

    // -- Symbol (index 39) --
    match obj(&data, elements, 39) {
        MarshalValue::Symbol(s) => {
            assert_eq!(s.as_slice(), b"test_symbol", "symbol at index 39");
        }
        other => panic!("expected Symbol at index 39, got {other:?}"),
    }

    // -- Strings (indices 40-43, Instance-wrapped) --
    assert_string(&data, elements, 40, b"hello world");
    assert_string(&data, elements, 41, b"");
    // Binary string (ASCII-8BIT) is not Instance-wrapped
    match obj(&data, elements, 42) {
        MarshalValue::String(s) => assert_eq!(s.as_slice(), b"binary\x00data"),
        other => panic!("expected String at index 42, got {other:?}"),
    }
    assert_string(&data, elements, 43, "\u{1F600}".as_bytes());

    // -- Regex (indices 44-45, Instance-wrapped) --
    {
        let (inner, _) = unwrap_instance(&data, elements, 44);
        match inner {
            MarshalValue::Regex { pattern, flags } => {
                assert_eq!(pattern.as_slice(), b"simple");
                assert_eq!(*flags, 0, "no flags");
            }
            other => panic!("expected Regex at index 44, got {other:?}"),
        }

        let (inner, _) = unwrap_instance(&data, elements, 45);
        match inner {
            MarshalValue::Regex { pattern, flags } => {
                assert_eq!(pattern.as_slice(), b"with flags");
                // Ruby regex flags: IGNORECASE=1, EXTENDED=2, MULTILINE=4
                assert_eq!(*flags, 1 | 2 | 4, "imx flags");
            }
            other => panic!("expected Regex at index 45, got {other:?}"),
        }
    }

    // -- Arrays (indices 46-47) --
    match obj(&data, elements, 46) {
        MarshalValue::Array(a) => assert_eq!(a.len(), 0, "empty array"),
        other => panic!("expected empty Array at index 46, got {other:?}"),
    }
    match obj(&data, elements, 47) {
        MarshalValue::Array(a) => {
            assert_eq!(a.len(), 4, "array with 4 elements");
            assert!(matches!(data.object(a[0]), MarshalValue::Fixnum(1)));
            // a[1] is Instance-wrapped string "two"
            match data.object(a[1]) {
                MarshalValue::Instance { inner, .. } => {
                    assert!(
                        matches!(data.object(*inner), MarshalValue::String(s) if s.as_slice() == b"two")
                    );
                }
                other => panic!("expected Instance(String) in array, got {other:?}"),
            }
            // a[2] is symbol :three (or SymbolLink if already seen)
            assert!(
                matches!(data.object(a[2]), MarshalValue::Symbol(s) if s.as_slice() == b"three")
                    || matches!(data.object(a[2]), MarshalValue::SymbolLink(_))
            );
            assert!(matches!(data.object(a[3]), MarshalValue::Float(v) if *v == 4.0));
        }
        other => panic!("expected Array at index 47, got {other:?}"),
    }

    // -- Hashes (indices 48-49) --
    match obj(&data, elements, 48) {
        MarshalValue::Hash(pairs) => assert_eq!(pairs.len(), 0, "empty hash"),
        other => panic!("expected empty Hash at index 48, got {other:?}"),
    }
    match obj(&data, elements, 49) {
        MarshalValue::Hash(pairs) => assert_eq!(pairs.len(), 3, "hash with 3 pairs"),
        other => panic!("expected Hash at index 49, got {other:?}"),
    }

    // -- HashDefault (index 50) --
    match obj(&data, elements, 50) {
        MarshalValue::HashDefault { pairs, default } => {
            assert_eq!(pairs.len(), 2, "hash_default with 2 pairs");
            assert!(
                matches!(data.object(*default), MarshalValue::Fixnum(42)),
                "default value should be 42"
            );
        }
        other => panic!("expected HashDefault at index 50, got {other:?}"),
    }

    // -- Object (index 51) --
    match obj(&data, elements, 51) {
        MarshalValue::Object { class, ivars } => {
            let class_name = data.symbol(*class).expect("class symbol");
            assert_eq!(class_name.as_slice(), b"TestObject");
            assert_eq!(ivars.len(), 2, "TestObject has 2 ivars");
        }
        other => panic!("expected Object at index 51, got {other:?}"),
    }

    // -- Struct (index 52) --
    match obj(&data, elements, 52) {
        MarshalValue::Struct { name, members } => {
            let struct_name = data.symbol(*name).expect("struct name symbol");
            assert_eq!(struct_name.as_slice(), b"TestStruct");
            assert_eq!(members.len(), 2, "TestStruct has 2 members");
            // Verify member values
            assert!(matches!(
                data.object(members[0].1),
                MarshalValue::Fixnum(100)
            ));
            assert!(matches!(
                data.object(members[1].1),
                MarshalValue::Fixnum(200)
            ));
        }
        other => panic!("expected Struct at index 52, got {other:?}"),
    }

    // -- Extended (index 53) --
    match obj(&data, elements, 53) {
        MarshalValue::Extended { module, inner } => {
            let module_name = data.symbol(*module).expect("module symbol");
            assert_eq!(module_name.as_slice(), b"TestModule");
            // inner should be an Object (TestObject)
            match data.object(*inner) {
                MarshalValue::Object { class, ivars } => {
                    let class_name = data.symbol(*class).expect("class symbol");
                    assert_eq!(class_name.as_slice(), b"TestObject");
                    assert_eq!(ivars.len(), 2);
                }
                other => panic!("expected Object inside Extended, got {other:?}"),
            }
        }
        other => panic!("expected Extended at index 53, got {other:?}"),
    }

    // -- Class (index 54) --
    match obj(&data, elements, 54) {
        MarshalValue::Class(name) => assert_eq!(name.as_slice(), b"String"),
        other => panic!("expected Class at index 54, got {other:?}"),
    }

    // -- Module (index 55) --
    match obj(&data, elements, 55) {
        MarshalValue::Module(name) => assert_eq!(name.as_slice(), b"Kernel"),
        other => panic!("expected Module at index 55, got {other:?}"),
    }

    // -- UserDefined (index 56, Instance-wrapped) --
    match obj(&data, elements, 56) {
        MarshalValue::Instance { inner, .. } => match data.object(*inner) {
            MarshalValue::UserDefined {
                class,
                data: payload,
            } => {
                let class_name = data.symbol(*class).expect("class symbol");
                assert_eq!(class_name.as_slice(), b"UserDefinedClass");
                assert_eq!(*payload, b"custom_payload");
            }
            other => panic!("expected UserDefined inside Instance at index 56, got {other:?}"),
        },
        other => panic!("expected Instance(UserDefined) at index 56, got {other:?}"),
    }

    // -- UserMarshal (index 57) --
    match obj(&data, elements, 57) {
        MarshalValue::UserMarshal { class, inner } => {
            let class_name = data.symbol(*class).expect("class symbol");
            assert_eq!(class_name.as_slice(), b"UserMarshalClass");
            // inner should be an Array [10, 20, 30]
            match data.object(*inner) {
                MarshalValue::Array(a) => {
                    assert_eq!(a.len(), 3);
                    assert!(matches!(data.object(a[0]), MarshalValue::Fixnum(10)));
                    assert!(matches!(data.object(a[1]), MarshalValue::Fixnum(20)));
                    assert!(matches!(data.object(a[2]), MarshalValue::Fixnum(30)));
                }
                other => panic!("expected Array inside UserMarshal, got {other:?}"),
            }
        }
        other => panic!("expected UserMarshal at index 57, got {other:?}"),
    }

    // -- UserString (index 58) --
    // Ruby wraps this as Instance { inner: UserString { class, inner: String } }
    match obj(&data, elements, 58) {
        MarshalValue::Instance { inner, .. } => match data.object(*inner) {
            MarshalValue::UserString { class, inner } => {
                let class_name = data.symbol(*class).expect("class symbol");
                assert_eq!(class_name.as_slice(), b"MyString");
                match data.object(*inner) {
                    MarshalValue::String(s) => {
                        assert_eq!(s.as_slice(), b"subclassed string");
                    }
                    other => panic!("expected String inside UserString, got {other:?}"),
                }
            }
            other => panic!("expected UserString inside Instance, got {other:?}"),
        },
        other => panic!("expected Instance(UserString) at index 58, got {other:?}"),
    }

    // -- ObjectReference (indices 59-60) --
    // Index 59: first occurrence of "shared" (Instance-wrapped String)
    assert_string(&data, elements, 59, b"shared");
    // Index 60: second occurrence should be an ObjectRef pointing back to the same object
    assert!(
        matches!(obj(&data, elements, 60), MarshalValue::ObjectRef(_)),
        "expected ObjectRef at index 60, got {:?}",
        obj(&data, elements, 60)
    );
}
