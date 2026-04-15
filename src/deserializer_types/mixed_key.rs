use serde::{Deserialize, Deserializer, de::Visitor};

/// Decently common key used in marshal data. Captures both fixnum and utf-8 str keys (if there's non utf8 in your keys idk man)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MixedKeyRef<'a> {
    Int(i32),
    Str(&'a str),
}

impl<'a> serde::Serialize for MixedKeyRef<'a> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            MixedKeyRef::Int(i) => i.serialize(serializer),
            MixedKeyRef::Str(s) => s.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for MixedKeyRef<'de> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MixedKeyVisitor;
        impl<'de> Visitor<'de> for MixedKeyVisitor {
            type Value = MixedKeyRef<'de>;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an integer or string")
            }

            fn visit_i32<E: serde::de::Error>(self, v: i32) -> Result<Self::Value, E> {
                Ok(MixedKeyRef::Int(v))
            }

            fn visit_borrowed_str<E: serde::de::Error>(
                self,
                v: &'de str,
            ) -> Result<Self::Value, E> {
                Ok(MixedKeyRef::Str(v))
            }
        }

        deserializer.deserialize_any(MixedKeyVisitor)
    }
}

impl std::fmt::Display for MixedKeyRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MixedKeyRef::Int(i) => i.fmt(f),
            MixedKeyRef::Str(s) => s.fmt(f),
        }
    }
}

/// Owned version of [`MixedKeyRef`]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MixedKey {
    Int(i32),
    Str(String),
}

impl From<MixedKeyRef<'_>> for MixedKey {
    fn from(value: MixedKeyRef<'_>) -> Self {
        match value {
            MixedKeyRef::Int(i) => Self::Int(i),
            MixedKeyRef::Str(s) => Self::Str(String::from(s)),
        }
    }
}

impl<'a> From<&'a MixedKey> for MixedKeyRef<'a> {
    fn from(value: &'a MixedKey) -> MixedKeyRef<'a> {
        match value {
            MixedKey::Int(i) => MixedKeyRef::Int(*i),
            MixedKey::Str(s) => MixedKeyRef::Str(s),
        }
    }
}

impl serde::Serialize for MixedKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            MixedKey::Int(i) => i.serialize(serializer),
            MixedKey::Str(s) => s.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for MixedKey {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct MixedKeyVisitor;
        impl<'de> Visitor<'de> for MixedKeyVisitor {
            type Value = MixedKey;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("an integer or string")
            }

            fn visit_i32<E: serde::de::Error>(self, v: i32) -> Result<Self::Value, E> {
                Ok(MixedKey::Int(v))
            }

            fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(MixedKey::Str(String::from(v)))
            }

            fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(MixedKey::Str(v))
            }
        }

        deserializer.deserialize_any(MixedKeyVisitor)
    }
}

impl std::fmt::Display for MixedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MixedKey::Int(i) => i.fmt(f),
            MixedKey::Str(s) => s.fmt(f),
        }
    }
}
