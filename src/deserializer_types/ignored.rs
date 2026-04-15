/// A type that discards any deserialized value.
///
/// Use as a type parameter when you don't care about a wrapper's extra data as you would serde::de::IgnoredAny
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Ignored;

impl<'de> serde_core::Deserialize<'de> for Ignored {
    fn deserialize<D: serde_core::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        serde_core::de::IgnoredAny::deserialize(deserializer)?;
        Ok(Ignored)
    }
}
