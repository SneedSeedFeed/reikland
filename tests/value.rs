// The byte strings below are Ruby 3.4's `Marshal.dump` output, inlined so the tests need neither
// Ruby nor generated files. The Ruby that produced each one is in the comment above it.
use std::sync::Arc;

use reikland::{Body, Object, Symbol, Value};
use serde::Deserialize;

// [nil, true, false, 0, 1, 122, 123, -1, -123, -124, 256, -257, 2**30 - 1, -2**30, 2**30,
//  -2**30 - 1, 2**31, 2**62, 2**64, -2**70]
const SCALARS: &[u8] = b"\x04\x08[\x190TFi\x00i\x06i\x7fi\x01{i\xfai\x80i\xff\x84i\x02\x00\x01i\xfe\xff\xfei\x04\xff\xff\xff?i\xfc\x00\x00\x00\xc0l+\x07\x00\x00\x00@l-\x07\x01\x00\x00@l+\x07\x00\x00\x00\x80l+\x09\x00\x00\x00\x00\x00\x00\x00@l+\x0a\x00\x00\x00\x00\x00\x00\x00\x00\x01\x00l-\x0a\x00\x00\x00\x00\x00\x00\x00\x00@\x00";

// [1.5, 1.5, 1e300, 1e300, -0.0, 0.0, 0.0, 100.0, Float::INFINITY, Float::NAN]
const FLOATS: &[u8] =
    b"\x04\x08[\x0ff\x081.5@\x06f\x0a1e300f\x0a1e300f\x07-0f\x060@\x0af\x081e2f\x08inff\x08nan";

// s = "shared"
// [s, s, "shared", "é".b, "日本".encode("Shift_JIS"), :sym, :sym, :"é", :"é"]
const STRINGS: &[u8] = b"\x04\x08[\x0eI\x22\x0bshared\x06:\x06ET@\x06I\x22\x0bshared\x06;\x00T\x22\x07\xc3\xa9I\x22\x09\x93\xfa\x96{\x06:\x0dencoding\x22\x0eShift_JIS:\x08sym;\x07I:\x07\xc3\xa9\x06;\x00T;\x08";

// Pt = Struct.new(:x, :y)
// [[1, [2]], {a: 1, "b" => [s]}, Hash.new(5).merge(k: 1), Pt.new(1, "y"), /ab+/mi]
const CONTAINERS: &[u8] = b"\x04\x08[\x0a[\x07i\x06[\x06i\x07{\x07:\x06ai\x06I\x22\x06b\x06:\x06ET[\x06I\x22\x0bshared\x06;\x06T}\x06:\x06ki\x06i\x0aS:\x07Pt\x07:\x06xi\x06:\x06yI\x22\x06y\x06;\x06TI/\x08ab+\x05\x06;\x06F";

// class Thing; def initialize = (@a = 1; @b = "two"); end
// class Name < String; end
// [Thing.new, Name.new("hi"), "x".extend(Tag), [].tap { _1.instance_variable_set(:@x, 1) },
//  String, Kernel]
const OBJECTS: &[u8] = b"\x04\x08[\x0bo:\x0aThing\x07:\x07@ai\x06:\x07@bI\x22\x08two\x06:\x06ETIC:\x09Name\x22\x07hi\x06;\x08TIe:\x08Tag\x22\x06x\x06;\x08TI[\x00\x06:\x07@xi\x06c\x0bStringm\x0bKernel";

// class Dumped  # _dump returns "data" with an ivar @inner = "ivar"
// class Boxed   # marshal_dump returns @v
// d = Dumped.new("data")
// [d, d, Boxed.new([1, s]), Time.at(0, in: "UTC")]
const USER: &[u8] = b"\x04\x08[\x09Iu:\x0bDumped\x09data\x07:\x06ET:\x0b@innerI\x22\x09ivar\x06;\x06T@\x07U:\x0aBoxed[\x07i\x06I\x22\x0bshared\x06;\x06TIu:\x09Time\x0d \x80\x11\xc0\x00\x00\x00\x00\x06:\x09zoneI\x22\x08UTC\x06;\x06F";

fn items(bytes: &[u8]) -> Vec<Value> {
    Value::from_bytes(bytes)
        .unwrap()
        .as_array()
        .unwrap()
        .to_vec()
}

fn same_object(a: &Value, b: &Value) -> bool {
    Arc::ptr_eq(&ref_of(a), &ref_of(b))
}

fn ref_of(value: &Value) -> Arc<Object> {
    match value {
        Value::Ref(object) => object.clone(),
        other => panic!("expected an object, got {other:?}"),
    }
}

#[test]
fn ruby_output_round_trips_byte_for_byte() {
    for (name, bytes) in [
        ("scalars", SCALARS),
        ("floats", FLOATS),
        ("strings", STRINGS),
        ("containers", CONTAINERS),
        ("objects", OBJECTS),
        ("user", USER),
    ] {
        let value = Value::from_bytes(bytes).unwrap();
        assert_eq!(value.to_bytes(), bytes, "{name}");
    }
}

#[test]
fn reads_integers_whichever_way_they_were_encoded() {
    let ints: Vec<_> = items(SCALARS)[3..16]
        .iter()
        .map(|v| v.as_int().unwrap())
        .collect();
    assert_eq!(
        ints,
        [
            0,
            1,
            122,
            123,
            -1,
            -123,
            -124,
            256,
            -257,
            (1 << 30) - 1,
            -(1 << 30),
            1 << 30,
            -(1 << 30) - 1
        ]
    );
    let scalars = items(SCALARS);
    assert_eq!(scalars[16], Value::Int(1 << 31));
    assert_eq!(scalars[17], Value::Int(1 << 62));
    let Body::BigInt(big) = &ref_of(&scalars[18]).body else {
        panic!("2**64 doesn't fit in an i64");
    };
    assert_eq!(big.to_string(), "18446744073709551616");
}

#[test]
fn reads_floats_and_their_links() {
    let floats = items(FLOATS);
    assert_eq!(floats[0], Value::Float(1.5));
    assert_eq!(floats[1], Value::Float(1.5), "link to a flonum");
    assert_eq!(floats[3], Value::Float(1e300));
    assert!(matches!(floats[4], Value::Float(f) if f == 0.0 && f.is_sign_negative()));
    assert!(matches!(floats[9], Value::Float(f) if f.is_nan()));
}

#[test]
fn keeps_strings_symbols_and_shared_objects_apart() {
    let strings = items(STRINGS);
    assert!(
        same_object(&strings[0], &strings[1]),
        "link to the same string"
    );
    assert!(
        !same_object(&strings[0], &strings[2]),
        "an equal but separate string"
    );
    assert_eq!(strings[0], strings[2]);
    assert_eq!(strings[0].as_str(), Some("shared"));
    assert_eq!(strings[0].ivar("E"), Some(&Value::Bool(true)));
    assert_eq!(
        strings[3].as_str(),
        Some("é"),
        "binary string with no encoding ivar"
    );
    assert!(ref_of(&strings[3]).ivars.is_empty());
    assert_eq!(strings[5].as_symbol(), Some("sym"));
    assert_eq!(strings[6].as_symbol(), Some("sym"));
    assert_eq!(strings[7].as_symbol(), Some("é"));
    assert_eq!(strings[8].as_symbol(), Some("é"));
}

#[test]
fn reads_containers() {
    let containers = items(CONTAINERS);
    let Body::Hash { pairs, default } = &ref_of(&containers[1]).body else {
        panic!("hash");
    };
    assert_eq!(pairs[0], (Value::symbol("a"), Value::Int(1)));
    assert_eq!(pairs[1].0.as_str(), Some("b"));
    assert_eq!(*default, None);
    let Body::Hash { default, .. } = &ref_of(&containers[2]).body else {
        panic!("hash with default");
    };
    assert_eq!(*default, Some(Value::Int(5)));
    let point = ref_of(&containers[3]);
    assert_eq!(point.class().map(Symbol::as_str), Some("Pt"));
    let Body::Struct { members, .. } = &point.body else {
        panic!("struct");
    };
    assert_eq!(members[1].1.as_str(), Some("y"));
    let regex = ref_of(&containers[4]);
    assert_eq!(
        regex.body,
        Body::Regex {
            source: b"ab+".to_vec(),
            flags: 5
        }
    );
}

#[test]
fn reads_objects_and_wrappers() {
    let objects = items(OBJECTS);
    assert_eq!(objects[0].ivar("@a"), Some(&Value::Int(1)));
    assert_eq!(objects[0].ivar("@b").and_then(Value::as_str), Some("two"));
    assert_eq!(ref_of(&objects[1]).user_class, Some(Symbol::from("Name")));
    assert_eq!(ref_of(&objects[2]).extended, [Symbol::from("Tag")]);
    assert_eq!(objects[3].ivar("@x"), Some(&Value::Int(1)));
    assert_eq!(ref_of(&objects[4]).body, Body::Class(b"String".to_vec()));
    assert_eq!(ref_of(&objects[5]).body, Body::Module(b"Kernel".to_vec()));
}

#[test]
fn dump_objects_claim_their_link_index_after_their_ivars() {
    // The link in USER is `@2`: the array is 0, the "ivar" string inside `d`'s ivars is 1, and only
    // then `d` itself is 2.
    let user = items(USER);
    assert!(same_object(&user[0], &user[1]));
    assert_eq!(
        ref_of(&user[0]).body,
        Body::UserDefined {
            class: "Dumped".into(),
            data: b"data".to_vec()
        }
    );
    assert_eq!(user[0].ivar("@inner").and_then(Value::as_str), Some("ivar"));
    let Body::UserMarshal { class, data } = &ref_of(&user[2]).body else {
        panic!("marshal_dump object");
    };
    assert_eq!(class.as_str(), "Boxed");
    assert_eq!(data.as_array().unwrap()[1].as_str(), Some("shared"));
}

#[test]
fn writes_shared_objects_as_links() {
    let shared = Value::string("once");
    let bytes = Value::array([shared.clone(), shared, Value::string("once")]).to_bytes();
    // Ruby: s = "once"; Marshal.dump([s, s, "once"])
    assert_eq!(
        bytes,
        b"\x04\x08[\x08I\x22\x09once\x06:\x06ET@\x06I\x22\x09once\x06;\x00T"
    );
}

#[test]
fn written_values_read_back_through_serde() {
    #[derive(Deserialize, Debug, PartialEq)]
    struct Move<'a> {
        #[serde(rename = "@id")]
        id: &'a str,
        #[serde(rename = "@pp")]
        pp: u8,
    }
    #[derive(Deserialize, Debug, PartialEq)]
    struct Pokemon<'a> {
        #[serde(rename = "@species")]
        species: &'a str,
        #[serde(rename = "@name")]
        name: Option<&'a str>,
        #[serde(rename = "@personalID")]
        personal_id: u32,
        #[serde(rename = "@scale")]
        scale: f64,
        #[serde(rename = "@moves", borrow)]
        moves: Vec<Move<'a>>,
    }

    let tackle = Value::object(
        "Pokemon::Move",
        [("@id", Value::symbol("TACKLE")), ("@pp", 35.into())],
    );
    let pokemon = Value::object(
        "Pokemon",
        [
            ("@species", Value::symbol("BULBASAUR")),
            ("@name", Value::string("Bulby")),
            ("@personalID", u32::MAX.into()),
            ("@scale", 1.25.into()),
            ("@moves", Value::array([tackle])),
        ],
    );
    let bytes = Value::array([pokemon, Value::Nil]).to_bytes();

    let read: Vec<Option<Pokemon>> =
        reikland::from_bytes_with_config(&bytes, reikland::DeserializerConfig::opinionated())
            .unwrap();
    assert_eq!(
        read,
        [
            Some(Pokemon {
                species: "BULBASAUR",
                name: Some("Bulby"),
                personal_id: u32::MAX,
                scale: 1.25,
                moves: vec![Move {
                    id: "TACKLE",
                    pp: 35
                }],
            }),
            None
        ]
    );
}

#[test]
fn rejects_cycles_bad_links_and_runaway_nesting() {
    // a = []; a << a; Marshal.dump(a)
    assert!(Value::from_bytes(b"\x04\x08[\x06@\x00").is_err());
    // A link to an object that was never read.
    assert!(Value::from_bytes(b"\x04\x08[\x06@\x07").is_err());
    assert!(Value::from_bytes(b"\x04\x08[\x06;\x00").is_err());
    // Truncated input.
    assert!(Value::from_bytes(b"\x04\x08[\x07i\x06").is_err());
    // An unsupported version.
    assert!(Value::from_bytes(b"\x04\x09\x30").is_err());

    // [[[[...nil...]]]], 10,000 deep.
    let deep = [b"\x04\x08".as_slice(), &b"[\x06".repeat(10_000), b"0"].concat();
    let error = Value::from_bytes(&deep).unwrap_err().to_string();
    assert!(error.contains("nested"), "{error}");
    // Wrappers only nest in a fixed order, so a run of them is rejected rather than recursed into.
    let wrappers = [b"\x04\x08".as_slice(), &b"I".repeat(10_000), b"\x22\x00"].concat();
    assert!(Value::from_bytes(&wrappers).is_err());
}

#[test]
fn existing_test_data_round_trips() {
    for (name, bytes) in [
        (
            "all_types",
            include_bytes!("../test_data/all_types.marshal").as_slice(),
        ),
        (
            "wrapper_types",
            include_bytes!("../test_data/wrapper_types.marshal"),
        ),
        ("species", include_bytes!("../test_data/species.dat")),
    ] {
        let value = Value::from_bytes(bytes).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(value.to_bytes() == bytes, "{name} didn't round trip");
    }
}
