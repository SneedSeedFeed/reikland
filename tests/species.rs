// DISCLAIMER: THIS TEST IS 100% AI GENERATED

use std::collections::HashMap;

use serde::Deserialize;
use serde::de::{self, Deserializer, IgnoredAny, MapAccess, Visitor};

use reikland::deserializer;
use reikland::types::rb_object::RbObject;

const SPECIES_DATA: &[u8] = include_bytes!("../test_data/species.dat");

/// The species hash maps both integer IDs and symbol names to the same species objects,
/// e.g. `{1 => species, :BULBASAUR => species, 2 => species, :IVYSAUR => species, ...}`.
/// The symbol-keyed entries are ObjectRefs back to the Object from the integer-keyed entry.
///
/// In this data, string fields are plain strings (no Instance wrapper), and symbol fields
/// deserialize as `&str` since the deserializer resolves symbols to their string content.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
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
    base_stats: HashMap<&'a str, i32>,
}

/// Hash key that accepts both integers and strings from Ruby's mixed-key hashes.
enum MixedKey<'a> {
    Int(i32),
    Str(#[allow(dead_code)] &'a str),
}

impl<'de> Deserialize<'de> for MixedKey<'de> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MixedKeyVisitor;
        impl<'de> Visitor<'de> for MixedKeyVisitor {
            type Value = MixedKey<'de>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an integer or string")
            }

            fn visit_i32<E: de::Error>(self, v: i32) -> Result<Self::Value, E> {
                Ok(MixedKey::Int(v))
            }

            fn visit_borrowed_str<E: de::Error>(self, v: &'de str) -> Result<Self::Value, E> {
                Ok(MixedKey::Str(v))
            }
        }

        deserializer.deserialize_any(MixedKeyVisitor)
    }
}

/// Deserialize only the integer-keyed species entries (symbol-keyed entries are ObjectRef
/// duplicates that may not resolve cleanly through the serde layer).
struct SpeciesById<'a>(HashMap<i32, Species<'a>>);

impl<'de> Deserialize<'de> for SpeciesById<'de> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ByIdVisitor;
        impl<'de> Visitor<'de> for ByIdVisitor {
            type Value = SpeciesById<'de>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a hash of species")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut by_id = HashMap::new();
                while let Some(key) = map.next_key::<MixedKey>()? {
                    match key {
                        MixedKey::Int(n) => {
                            let sp: RbObject<Species> = map.next_value()?;
                            by_id.insert(n, sp.fields);
                        }
                        MixedKey::Str(_) => {
                            map.next_value::<IgnoredAny>()?;
                        }
                    }
                }
                Ok(SpeciesById(by_id))
            }
        }

        deserializer.deserialize_map(ByIdVisitor)
    }
}

fn parse_species<'a>(data: &'a reikland::marshal::MarshalData<'a>) -> HashMap<i32, Species<'a>> {
    let db: SpeciesById =
        deserializer::from_marshal_data(data).expect("failed to deserialize species");
    db.0
}

#[test]
fn parse_species_dat() {
    let data = reikland::marshal::parse(SPECIES_DATA).expect("failed to parse species.dat");
    let species = parse_species(&data);

    assert!(
        species.len() > 400,
        "expected hundreds of species, got {}",
        species.len()
    );

    // Spot-check Bulbasaur by numeric ID
    let bulbasaur = species.get(&1).expect("missing species #1 (Bulbasaur)");
    assert_eq!(bulbasaur.id, "BULBASAUR");
    assert_eq!(bulbasaur.real_name, "Bulbasaur");
    assert_eq!(bulbasaur.type1, "GRASS");
    assert_eq!(bulbasaur.type2, "POISON");
    assert_eq!(bulbasaur.real_category, "Seed");
    assert_eq!(bulbasaur.form, 0);
    assert_eq!(bulbasaur.base_stats["HP"], 45);
    assert_eq!(bulbasaur.base_stats["ATTACK"], 49);
    assert_eq!(bulbasaur.base_stats["DEFENSE"], 49);
    assert_eq!(bulbasaur.base_stats["SPEED"], 45);
}

#[test]
fn species_id_matches_key() {
    let data = reikland::marshal::parse(SPECIES_DATA).expect("failed to parse species.dat");
    let species = parse_species(&data);

    for (&key, sp) in &species {
        assert_eq!(
            key, sp.id_number,
            "hash key {} does not match species id_number {} ({})",
            key, sp.id_number, sp.id
        );
    }
}

#[test]
fn all_species_have_base_stats() {
    let data = reikland::marshal::parse(SPECIES_DATA).expect("failed to parse species.dat");
    let species = parse_species(&data);

    let expected_stats = [
        "HP",
        "ATTACK",
        "DEFENSE",
        "SPECIAL_ATTACK",
        "SPECIAL_DEFENSE",
        "SPEED",
    ];

    for sp in species.values() {
        for &stat in &expected_stats {
            assert!(
                sp.base_stats.contains_key(stat),
                "species {} missing base stat {}",
                sp.id,
                stat
            );
        }
    }
}
