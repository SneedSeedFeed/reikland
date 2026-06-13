# reikland
A ruby marshal deserializer that's compatible with the normal `serde::Deserialize` trait. If you don't need that compatibility you probably want the wonderful [alox-48](https://crates.io/crates/alox-48) which was the inspiration to try this in the first place.

## Read this before deciding to use this crate so you understand the why and how to use it properly
I found the marshal format to have a good degree of desync between the "intended" and "literal" ways to deserialize a value in Rust. For example: An instance variable is basically just a `(T, HashMap<Symbol, Value>)` but in many cases you (the lovely person reading this) just want `T`. However if I just made instance variables deserialize to `T` we are losing information, so you get to pick:

- `from_bytes` deserializes "literally", losing nothing for better or worse. There's some wrapper types that should help get at these inner values
- `from_bytes_with_config` takes a `DeserializerConfig` whose flags flatten the wrappers you don't care about into their useful contents, so plain Rust types just work. `DeserializerConfig::opinionated()` turns them all on:

```rust
#[derive(Deserialize)]
struct Player<'a> {
    #[serde(rename = "@name")]
    name: &'a str, // the ivar wrapper around the string is flattened away
    #[serde(rename = "@level")]
    level: i32,
}

// the Object's class name is skipped too
let player: Player = from_bytes_with_config(data, DeserializerConfig::opinionated())?;
```

The wrapper types for the literal route. Every marshal type that comes through as a sequence leads with its useful value (inner value, fields, payload...) followed by the extra information (ivars, class name...), so `Transparent` always grabs the right element:

```rust
struct Transparent<T>(pub T); // Will deserialize T, but if it runs into a sequence (such as an ivar) it will try to take the first member as T
struct TransparentOpt<T, O>(pub T, pub Option<O>) // Same as Transparent<T> but also captures the second value of a sequence if possible

// Deserialize an instance variable wrapper as (inner_value, ivars_map).
// Encoding pulls out the encoding specifically. WithEncoding<T> = Ivar<T, Encoding>.
struct Ivar<T, O = Ignored> { pub inner: T, pub ivars: O }
struct Encoding(pub RubyEncoding);

// Deserialize a Ruby Object/Struct as (fields_map, class_name). RbStruct is an alias to the same
struct RbObject<T, N = Ignored> { pub class: N, pub fields: T }

// Deserialize a Ruby Regex as its pattern and flags byte.
struct RbRegex<P = String> { pub pattern: P, pub flags: u8 }

// Deserialize a Ruby Hash-with-default as the hash and its default value.
struct RbHashDefault<T, D = Ignored> { pub hash: T, pub default: D }

// Ruby hashes often have both integer and symbol keys pointing at the same data.
enum MixedKeyRef<'a> { Int(i32), Str(&'a str) }
enum MixedKey { Int(i32), Str(String) } // owned variant

// These take a mixed-key hash and keep only one side:
struct DualKeyMap<K, V>(pub HashMap<K, V>); // keeps string keys, discards int keys
struct DualKeyMapInt<K, V>(pub HashMap<K, V>); // keeps int keys, discards string keys
struct DualKeyVec<T>(pub Vec<T>); // keeps int-keyed values in insertion order
struct DualKeyVecSparse<T>(pub Vec<T>); // keeps int-keyed values indexed by their key (no holes allowed)
struct DualKeyVecSparseHoley<T>(pub Vec<Option<T>>); // keeps int-keyed values indexed by their key and allows holes by leaving Option::None
```

All the struct types above implement `Deref`/`DerefMut` to their "main" field (the first generic) so you can use them without unwrapping in most cases.

In practice this looks like
```rust
// make your types as per usual
#[derive(Debug, Clone, Hash, PartialEq, Eq, Deserialize)]
struct Species<'a> {
    #[serde(rename = "@id")]
    id: &'a str,
    #[serde(rename = "@id_number")]
    id_number: i32,
    #[serde(rename = "@species")]
    species: &'a str,
    #[serde(rename = "@form")]
    form: i32,
    #[serde(rename = "@real_name")]
    real_name: &'a str,
    #[serde(rename = "@real_category")]
    real_category: &'a str,
    #[serde(rename = "@type1")]
    type1: &'a str,
    #[serde(rename = "@type2")]
    type2: &'a str,
    #[serde(rename = "@base_stats")]
    base_stats: BaseStats,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize)]
struct BaseStats {
    #[serde(rename = "HP")]
    hp: u16,
    #[serde(rename = "ATTACK")]
    atk: u16,
    #[serde(rename = "DEFENSE")]
    def: u16,
    #[serde(rename = "SPECIAL_ATTACK")]
    spa: u16,
    #[serde(rename = "SPECIAL_DEFENSE")]
    spd: u16,
    #[serde(rename = "SPEED")]
    spe: u16,
}

fn parse_species(data: &[u8]) -> Vec<RbObject<Species<'_>>> {
    // use `reikland::deserializer_types` to help cut through the chaff
    // DualKeyVec takes a Map<I32OrString, Value> and just pushes the i32 keys into a vec
    let db: DualKeyVec<RbObject<Species>> =
        deserializer::from_bytes(data).expect("failed to deserialize species");
    db.0
}
```

### What happened to the exposed parser?
I put it down. It wasn't very good and was kinda buggy (along with most of v0.1)

#### Why does the crate's name suck?
I was playing a lot of Total War Warhammer 3 when I first decided to make it. Something something marshal my men, summon the elector counts...