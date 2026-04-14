// DISCLAIMER: THIS TEST IS 80% AI GENERATED

use itertools::Itertools;
use serde::Deserialize;

use reikland::RbObject;
use reikland::deserializer;
use reikland::deserializer_types::dual_key_map::DualKeyVec;

const SPECIES_DATA: &[u8] = include_bytes!("../test_data/species.dat");

/// The species hash maps both integer IDs and symbol names to the same species objects,
/// e.g. `{1 => species, :BULBASAUR => species, 2 => species, :IVYSAUR => species, ...}`.
/// The symbol-keyed entries are ObjectRefs back to the Object from the integer-keyed entry.
///
/// In this data, string fields are plain strings (no Instance wrapper), and symbol fields
/// deserialize as `&str` since the deserializer resolves symbols to their string content.
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
    let db: DualKeyVec<RbObject<Species>> =
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

    // Spot-check Bulbasaur (first entry, integer key 1)
    let bulbasaur = &species[0];
    assert_eq!(bulbasaur.id, "BULBASAUR");
    assert_eq!(bulbasaur.real_name, "Bulbasaur");
    assert_eq!(bulbasaur.type1, "GRASS");
    assert_eq!(bulbasaur.type2, "POISON");
    assert_eq!(bulbasaur.real_category, "Seed");
    assert_eq!(bulbasaur.form, 0);
    assert_eq!(bulbasaur.base_stats.hp, 45);
    assert_eq!(bulbasaur.base_stats.atk, 49);
    assert_eq!(bulbasaur.base_stats.def, 49);
    assert_eq!(bulbasaur.base_stats.spe, 45);
}

#[test]
fn species_ids_are_strictly_increasing() {
    let data = reikland::marshal::parse(SPECIES_DATA).expect("failed to parse species.dat");
    let species = parse_species(&data);

    for window in species.windows(2) {
        assert!(
            window[0].id_number < window[1].id_number,
            "species id_number {} ({}) should be less than {} ({})",
            window[0].id_number,
            window[0].id,
            window[1].id_number,
            window[1].id,
        );
    }
}

#[test]
fn all_species_are_unique() {
    let data = reikland::marshal::parse(SPECIES_DATA).expect("failed to parse species.dat");
    let species = parse_species(&data);

    let unique = species.clone().into_iter().unique().collect::<Vec<_>>();

    assert_eq!(species.len(), unique.len())
}
