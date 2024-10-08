use std::time::Duration;

use serde::{Deserialize, Deserializer};

use crate::{serde::deserialize_boxed_string, service::Configurations};

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) struct Configuration {
    #[serde(
        deserialize_with = "deserialize_seconds",
        rename = "refresh_seconds"
    )]
    pub refresh_period: Duration,
    pub verbose_output: bool,
    #[serde(default, deserialize_with = "deserialize_boxed_string")]
    pub prepend: Box<str>,
    pub services: Configurations,
}

fn deserialize_seconds<'de, D>(deserialize: D) -> Result<Duration, D::Error>
where
    D: Deserializer<'de>,
{
    u64::deserialize(deserialize).map(Duration::from_secs)
}
