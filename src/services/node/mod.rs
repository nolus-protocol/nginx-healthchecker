use std::sync::Arc;

use anyhow::Result;
use reqwest::{Client as ReqwestClient, Response as ReqwestResponse, Url};
use serde::Deserialize;
use tokio::sync::Mutex;
use tracing::info;

use crate::{
    http_client::http_client,
    serde::deserialize_boxed_string,
    service::{self, Instance, InstanceName, OutputVerbosity, Status},
    state::Epoch,
};

use self::status_response::StatusResponse;

mod status_response;

pub(crate) struct Instances<T> {
    pub lcd: T,
    pub json_rpc: T,
    pub grpc: T,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) struct StorageConfiguration {
    json_rpc_url: Url,
    outputs: Outputs,
}

impl StorageConfiguration {
    pub async fn create_instance(
        self,
        instance_name: Box<str>,
    ) -> Result<Instances<Instance<Configuration, State>>> {
        let json_rpc = http_client()?;

        let url = self.json_rpc_url.clone().join("/status")?;

        let last_block = State::fetch_status(&json_rpc, url.clone())
            .await?
            .latest_block_height();

        let enabled = Status::Disabled;

        let mutable = Mutex::new(StateInnerMutable {
            last_block,
            epoch: Epoch::new(),
            status: enabled,
        });

        let state = State(Arc::new(StateInner {
            json_rpc,
            url,
            mutable,
        }));

        Ok(Instances {
            lcd: Instance::new(
                instance_name.clone(),
                enabled,
                Configuration {
                    output: self.outputs.lcd,
                },
                state.clone(),
            ),
            json_rpc: Instance::new(
                instance_name.clone(),
                enabled,
                Configuration {
                    output: self.outputs.json_rpc,
                },
                state.clone(),
            ),
            grpc: Instance::new(
                instance_name,
                enabled,
                Configuration {
                    output: self.outputs.grpc,
                },
                state.clone(),
            ),
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
struct Outputs {
    #[serde(deserialize_with = "deserialize_boxed_string")]
    lcd: Box<str>,
    #[serde(deserialize_with = "deserialize_boxed_string")]
    json_rpc: Box<str>,
    #[serde(deserialize_with = "deserialize_boxed_string")]
    grpc: Box<str>,
}

#[repr(transparent)]
pub(crate) struct Configuration {
    output: Box<str>,
}

impl service::Configuration for Configuration {
    fn output(&self) -> &str {
        &self.output
    }
}

#[derive(Clone)]
#[repr(transparent)]
pub(crate) struct State(Arc<StateInner>);

impl State {
    async fn fetch_status(
        json_rpc: &ReqwestClient,
        url: Url,
    ) -> Result<StatusResponse> {
        json_rpc
            .get(url)
            .send()
            .await
            .and_then(ReqwestResponse::error_for_status)?
            .json()
            .await
            .map_err(From::from)
    }
}

impl service::Healthcheck for State {
    async fn healthcheck(
        &mut self,
        epoch: Epoch,
        output_verbosity: OutputVerbosity<InstanceName<'_>>,
    ) -> Status {
        let state = &*self.0;

        let mut lock = state.mutable.lock().await;

        if lock.epoch != epoch {
            lock.status =
                Self::fetch_status(&state.json_rpc, state.url.clone())
                    .await
                    .map_or(
                        Status::Disabled,
                        #[inline]
                        |response| {
                            let latest_block_height =
                                response.latest_block_height();

                            if lock.last_block < latest_block_height {
                                lock.last_block = latest_block_height;

                                (!response.catching_up()).into()
                            } else {
                                Status::Disabled
                            }
                        },
                    );

            lock.epoch = epoch;
        };

        if let OutputVerbosity::Verbose(InstanceName {
            service_name,
            instance_name,
        }) = output_verbosity
        {
            info!(
                "[{service_name:?}; {instance_name:?}] is {status}.",
                status = match lock.status {
                    Status::Disabled => "DOWN",
                    Status::Enabled => "UP",
                }
            );
        }

        lock.status
    }
}

struct StateInner {
    json_rpc: ReqwestClient,
    url: Url,
    mutable: Mutex<StateInnerMutable>,
}

struct StateInnerMutable {
    last_block: u64,
    epoch: Epoch,
    status: Status,
}
