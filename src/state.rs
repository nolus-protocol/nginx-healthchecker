use std::{path::Path, process::Command, time::Duration};

use anyhow::{anyhow, Context as _, Result};
use futures::{stream::FuturesUnordered, StreamExt as _};
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt as _,
};
use tracing::{error, info};

use crate::{
    configuration::Configuration,
    service::{
        self, OutputVerbosity, Service, ServiceName, Services, StateChange,
        WriteOutStatus,
    },
    writer,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub(super) struct Epoch(u8);

impl Epoch {
    pub const fn new() -> Self {
        const { Epoch(0) }
    }

    #[inline]
    fn start_new_epoch(&mut self) {
        self.0 = self.0.wrapping_add(1);
    }
}

pub(super) struct State {
    static_configuration: Box<[u8]>,
    refresh_period: Duration,
    verbose_output: bool,
    global_prepend: Box<str>,
    epoch: Epoch,
    services: Services,
}

impl State {
    pub async fn load(
        static_configuration: &Path,
        services_configuration: &Path,
    ) -> Result<Self> {
        info!("Loading configuration.");

        let static_configuration =
            Self::load_static_configuration(static_configuration).await?;

        let Configuration {
            refresh_period,
            verbose_output,
            prepend: global_prepend,
            services,
        } = Self::load_services_configuration(services_configuration).await?;

        info!("Preparing service clients.");

        service::from_configurations(services)
            .await
            .inspect(|_| {
                info!("Prepared service clients.");
            })
            .inspect_err(|error| {
                error!(
                    ?error,
                    "Failed to prepare service clients! Cause: {error}",
                )
            })
            .map(|services| Self {
                static_configuration,
                refresh_period,
                verbose_output,
                global_prepend,
                epoch: Epoch::new(),
                services,
            })
    }

    async fn load_static_configuration(
        static_configuration: &Path,
    ) -> Result<Box<[u8]>> {
        info!("Loading static configuration.");

        fs::read(static_configuration)
            .await
            .map(Vec::into_boxed_slice)
            .context("Failed to load static configuration!")
            .inspect(|_| {
                info!("Loaded static configuration.");
            })
            .inspect_err(|error| {
                error!(
                    ?error,
                    "Failed to load static configuration! Cause: {error}",
                );
            })
    }

    async fn load_services_configuration(
        services_configuration: &Path,
    ) -> Result<Configuration> {
        info!("Loading services configuration.");

        let services_json = fs::read(services_configuration)
            .await
            .map(Vec::into_boxed_slice)
            .context("Failed to load services configuration!")
            .inspect(|_| {
                info!("Loaded services configuration.");
            })
            .inspect_err(|error| {
                error!(
                    ?error,
                    "Failed to load services configuration! Cause: {error}",
                );
            })?;

        serde_json::from_reader(&*services_json)
            .context("Failed to parse services configuration!")
            .map_err(From::from)
    }

    #[inline]
    pub const fn refresh_period(&self) -> Duration {
        self.refresh_period
    }

    pub async fn output_configuration(
        &mut self,
        output_configuration: &Path,
        forced: bool,
    ) -> Result<()> {
        let updated = self
            .services
            .iter_mut()
            .map(|(service_name, service)| {
                service.healthcheck(
                    self.epoch,
                    if self.verbose_output {
                        OutputVerbosity::Verbose(ServiceName { service_name })
                    } else {
                        OutputVerbosity::Standard
                    },
                )
            })
            .collect::<FuturesUnordered<_>>()
            .fold(StateChange::Unchanged, |accumulated, service| async move {
                accumulated & service
            })
            .await;

        self.epoch.start_new_epoch();

        if forced || matches!(updated, StateChange::Changed) {
            self.write_out_services(output_configuration).await?;

            if Command::new("systemctl")
                .arg("reload")
                .arg("nginx.service")
                .status()
                .context(
                    "Failed to invoke Systemd reload command for `nginx`!",
                )?
                .success()
            {
                Ok(())
            } else {
                Err(anyhow!(
                    "Systemd reload command for `nginx` exited with an error!"
                ))
            }
        } else {
            Ok(())
        }
    }

    async fn write_out_services(
        &mut self,
        output_configuration: &Path,
    ) -> Result<()> {
        let mut output_configuration =
            File::create(output_configuration).await?;

        output_configuration
            .write_all(&self.static_configuration)
            .await?;

        for (service_name, service) in &self.services {
            Self::write_out_service(
                &mut output_configuration,
                &self.global_prepend,
                service_name,
                service,
            )
            .await?;
        }

        info!("");

        Ok(())
    }

    async fn write_out_service(
        output_configuration: &mut File,
        global_prepend: &str,
        service_name: &Box<str>,
        service: &Service,
    ) -> Result<()> {
        output_configuration.write_all(b"\nupstream ").await?;

        output_configuration
            .write_all(service_name.as_bytes())
            .await?;

        output_configuration.write_all(b" {\n").await?;

        let WriteOutStatus { healthy_instances } = service
            .write_out(
                writer::UpstreamSectionEntry::new(&mut *output_configuration),
                global_prepend,
            )
            .await?;

        info!(
            "{service_name:?} has {healthy_instances} healthy instance{}.",
            if healthy_instances == 1 { "" } else { "s" }
        );

        output_configuration
            .write_all(b"}\n")
            .await
            .map_err(From::from)
    }
}
