use serde::{Deserialize, Deserializer};

pub(crate) fn deserialize_boxed_string<'de, D>(
    deserialize: D,
) -> Result<Box<str>, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserialize).map(|value| value.trim().into())
}
