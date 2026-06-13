/// Deserialization config to allow bypassing of wrappers such as ivars.
///
/// By default every flag is off and values deserialize "literally": wrapper types like
/// instance variables come through as tuples, which the [`deserializer_types`][crate::deserializer_types] wrappers can help pick apart.
///
/// Each flag trades completeness for convenience by deserializing a marshal type as just its
/// "useful" contents (with useful being kinda subjective).
/// With all flags on (see [`Self::opinionated`]) plain Rust types usually just work.
///
/// ```no_run
/// use serde::Deserialize;
/// use reikland::{DeserializerConfig, from_bytes_with_config};
///
/// #[derive(Deserialize)]
/// struct Player<'a> {
///     #[serde(rename = "@name")]
///     name: &'a str, // an ivar-wrapped string
///     #[serde(rename = "@level")]
///     level: i32,
/// }
///
/// let data: &[u8] = todo!("some .rxdata / Marshal.dump file");
/// // the Object's class name is also skipped
/// let player: Player = from_bytes_with_config(data, DeserializerConfig::opinionated()).unwrap();
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeserializerConfig {
    /// Deserialize an instance variable as its inner value, skipping the ivars
    pub ivar_as_inner: bool,
    /// Deserialize an Object or Struct as a map of its fields, skipping the class name.
    pub object_as_map: bool,
    /// Deserialize the class-carrying wrappers Extended, UserClass, UserMarshal and Data as their inner value, skipping the class/module name.
    pub classed_as_inner: bool,
    /// Deserialize a Hash with a default value as a plain map, skipping the default.
    pub hash_default_as_map: bool,
}
// swap the bools for a bitfield?

impl DeserializerConfig {
    /// Create a new config in strict mode, all types are deserialized literally
    pub const fn new() -> Self {
        Self {
            ivar_as_inner: false,
            object_as_map: false,
            classed_as_inner: false,
            hash_default_as_map: false,
        }
    }

    /// Create a new config in opinionated mode, every wrapper deserializes as its useful contents
    pub const fn opinionated() -> Self {
        Self {
            ivar_as_inner: true,
            object_as_map: true,
            classed_as_inner: true,
            hash_default_as_map: true,
        }
    }

    pub const fn with_ivar_as_inner(mut self, on: bool) -> Self {
        self.ivar_as_inner = on;
        self
    }

    pub const fn with_object_as_map(mut self, on: bool) -> Self {
        self.object_as_map = on;
        self
    }

    pub const fn with_classed_as_inner(mut self, on: bool) -> Self {
        self.classed_as_inner = on;
        self
    }

    pub const fn with_hash_default_as_map(mut self, on: bool) -> Self {
        self.hash_default_as_map = on;
        self
    }
}
