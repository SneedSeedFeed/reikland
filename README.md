# reikland
A ruby marshal parser and deserializer that's compatible with the normal `serde::Deserialize` trait. If you don't need that compatibility you probably want the wonderful [alox-48](https://crates.io/crates/alox-48) which was the inspiration to try this in the first place.

## Read this before deciding to use this crate so you understand the why and how to use it properly
I found the marshal format to have a good degree of desync between the "intended" and "literal" ways to deserialize a value in Rust. For example: An instance variable is basically just a `(T, HashMap<Symbol, Value>)` but in many cases you (the lovely person reading this) just want `T`. However if I just made instance variables deserialize to `T` we are losing information so I made the executive decision to provide a collection of helpful wrappers to get at `T` with less pain.

```rust
struct Transparent<T>(pub T); // Will deserialize T, but if it runs into a sequence (such as an ivar) it will try to take the first member as T
struct TransparentOpt<T, O>(pub T, pub Option<O>) // Same as Transparent<T> but also captures the second value of a sequence if possible

// Deserialize an instance variable wrapper as (inner_value, ivars_map).
// Encoding pulls out the encoding specifically. WithEncoding<T> = Ivar<T, Encoding>.
struct Ivar<T, O = ()> { pub inner: T, pub ivars: O }
struct Encoding(pub RubyEncoding);

// Deserialize a Ruby Object/Struct as (class_name, fields_map). RbStruct is an alias to the same
struct RbObject<T, N = ()> { pub class: N, pub fields: T }

// Deserialize a Ruby Regex as its pattern and flags byte.
struct RbRegex<P = String> { pub pattern: P, pub flags: u8 }

// Deserialize a Ruby Hash-with-default as the hash and its default value.
struct RbHashDefault<T, D = ()> { pub hash: T, pub default: D }

// Ruby hashes often have both integer and symbol keys pointing at the same data.
enum MixedKeyRef<'a> { Int(i32), Str(&'a str) }
enum MixedKey { Int(i32), Str(String) } // owned variant

// These take a mixed-key hash and keep only one side:
struct DualKeyMap<K, V>(pub HashMap<K, V>); // keeps string keys, discards int keys
struct DualKeyMapInt<K, V>(pub HashMap<K, V>); // keeps int keys, discards string keys
struct DualKeyVec<T>(pub Vec<T>); // keeps int-keyed values in insertion order
struct DualKeyVecSparse<T>(pub Vec<T>); // keeps int-keyed values indexed by their key (no holes allowed)
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

fn parse_species<'a>(data: &'a reikland::marshal::MarshalData<'a>) -> Vec<RbObject<Species<'a>>> {
    // use `reikland::deserializer_types` to help cut through the chaff
    // DualKeyVec takes a Map<I32OrString, Value> and just pushes the i32 keys into a vec
    let db: DualKeyVec<RbObject<Species>> =
        deserializer::from_marshal_data(data).expect("failed to deserialize species");
    db.0
}
```

### I don't like what you've done with the place
~~me neither~~ That's fine! The parsing logic is exposed via `reikland::marshal::parse` which is used internally by the `serde` implementation so if you want to handle things yourself from the raw data go nuts. I parse the entire marshal object first before deserializing since it was simpler for me, and everything is stored flat with no scary recursive types.

```rust
#[derive(Debug)]
pub struct MarshalData<'a> {
    pub version: VersionNumber,
    pub symbols: SymbolTable<'a>,
    pub objects: ObjectTable<'a>,
    pub root: ObjectIdx,
}
```

#### Why does the crate's name suck?
I was playing a lot of Total War Warhammer 3 when I first decided to make it. Something something marshal my men, summon the elector counts...