use reqwest::Url;
use serde::de::{Deserialize as _, Deserializer, Error as _};

pub(crate) fn deserialize<'de, D>(deserializer: D) -> Result<Url, D::Error>
where
    D: Deserializer<'de>,
{
    String::deserialize(deserializer)
        .and_then(|url| url.parse().map_err(D::Error::custom))
}
