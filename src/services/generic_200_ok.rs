use anyhow::Result;
use reqwest::{Client as ReqwestClient, Response as ReqwestResponse, Url};
use serde::Deserialize;
use tracing::info;

use crate::{
    http_client::http_client,
    serde::deserialize_boxed_string,
    service::{self, Instance, InstanceName, OutputVerbosity, Status},
    state::Epoch,
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(crate) struct StorageConfiguration {
    #[serde(with = "crate::url")]
    healthcheck_url: Url,
    #[serde(deserialize_with = "deserialize_boxed_string")]
    output: Box<str>,
}

impl StorageConfiguration {
    pub(crate) async fn create_instance(
        self,
        instance_name: Box<str>,
    ) -> Result<Instance<Configuration, State>> {
        let client = http_client()?;

        let state = State {
            client,
            healthcheck_url: self.healthcheck_url,
        };

        Ok(Instance::new(
            instance_name,
            state.healthcheck().await,
            Configuration {
                output: self.output,
            },
            state,
        ))
    }
}

#[derive(Clone)]
pub(crate) struct Configuration {
    output: Box<str>,
}

impl service::Configuration for Configuration {
    fn output(&self) -> &str {
        &self.output
    }
}

#[derive(Clone)]
pub(crate) struct State {
    client: ReqwestClient,
    healthcheck_url: Url,
}

impl State {
    async fn healthcheck(&self) -> Status {
        self.client
            .get(self.healthcheck_url.clone())
            .send()
            .await
            .and_then(ReqwestResponse::error_for_status)
            .is_ok()
            .into()
    }
}

impl service::Healthcheck for State {
    #[inline]
    async fn healthcheck(
        &mut self,
        _: Epoch,
        output_verbosity: OutputVerbosity<InstanceName<'_>>,
    ) -> Status {
        let status = Self::healthcheck(self).await;

        if let OutputVerbosity::Verbose(InstanceName {
            service_name,
            instance_name,
        }) = output_verbosity
        {
            info!(
                "[{service_name:?}; {instance_name:?}] is {status}.",
                status = match status {
                    Status::Disabled => "DOWN",
                    Status::Enabled => "UP",
                }
            );
        }

        status
    }
}
