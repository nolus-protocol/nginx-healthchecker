use serde::{
    de::{Deserializer, Error as DeserializeError},
    Deserialize,
};

pub(super) struct StatusResponse {
    latest_block_height: u64,
    catching_up: bool,
}

impl StatusResponse {
    #[inline]
    pub const fn latest_block_height(&self) -> u64 {
        self.latest_block_height
    }

    #[inline]
    pub const fn catching_up(&self) -> bool {
        self.catching_up
    }
}

impl<'de> Deserialize<'de> for StatusResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Root {
            result: ResultField,
        }

        #[derive(Deserialize)]
        struct ResultField {
            sync_info: SyncInfoField,
        }

        #[derive(Deserialize)]
        struct SyncInfoField {
            latest_block_height: Box<str>,
            catching_up: bool,
        }

        Root::deserialize(deserializer).and_then(
            #[inline]
            |Root {
                 result:
                     ResultField {
                         sync_info:
                             SyncInfoField {
                                 latest_block_height,
                                 catching_up,
                             },
                     },
             }| {
                latest_block_height
                    .parse()
                    .map(|latest_block_height| Self {
                        latest_block_height,
                        catching_up,
                    })
                    .map_err(DeserializeError::custom)
            },
        )
    }
}
