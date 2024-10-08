use std::{
    collections::btree_map::{BTreeMap, Entry as BTreeMapEntry},
    future::Future,
};

use anyhow::{anyhow, Result};
use futures::{stream::FuturesUnordered, StreamExt as _, TryStreamExt as _};

use crate::{
    services::{generic_200_ok, node},
    state::Epoch,
};

pub(crate) use self::{
    configuration::Configurations,
    instance::{
        Configuration, Healthcheck, Instance, InstanceName, OutputVerbosity,
        ServiceName, StateChange, Status,
    },
};

mod configuration;
mod instance;

pub(crate) async fn from_configurations(
    configurations: Configurations,
) -> Result<Services> {
    let mut services = BTreeMap::new();

    for (service_name, configuration) in configurations {
        match configuration {
            configuration::Configuration::Generic200Ok {
                prepend,
                instances,
            } => {
                generic_service(prepend, instances)
                    .await
                    .map(|service| [(service_name, service)])
                    .and_then(collect_services_from_iter(&mut services))?;
            }
            configuration::Configuration::Node { prepend, instances } => {
                node_services(service_name.into_string(), prepend, instances)
                    .await
                    .and_then(collect_services_from_iter(&mut services))?;
            }
        }
    }

    Ok(services)
}

pub(crate) type Services = BTreeMap<Box<str>, Service>;

pub(crate) struct Service {
    prepend: Box<str>,
    instances: Instances,
}

macro_rules! define_instances {
    ($visibility:vis enum $enum:ident {
        $( $variant:ident < $configuration:ty , $state:ty $(,)? > ),+ $( , )?
    }) => {
        $visibility enum $enum {
            $($variant(
                Box<[Instance<$configuration, $state>]>,
            ),)+
        }
    };
}

define_instances! {
    pub(crate) enum Instances {
        Generic200Ok<generic_200_ok::Configuration, generic_200_ok::State>,
        Node<node::Configuration, node::State>,
    }
}

impl Instances {
    #[inline]
    pub async fn healthcheck(
        &mut self,
        epoch: Epoch,
        output_verbosity: OutputVerbosity<ServiceName<'_>>,
    ) -> StateChange {
        match self {
            Self::Generic200Ok(instances) => {
                Self::healthcheck_instances(instances, epoch, output_verbosity)
                    .await
            }
            Self::Node(instances) => {
                Self::healthcheck_instances(instances, epoch, output_verbosity)
                    .await
            }
        }
    }

    async fn healthcheck_instances<C, S>(
        instances: &mut [Instance<C, S>],
        epoch: Epoch,
        output_verbosity: OutputVerbosity<ServiceName<'_>>,
    ) -> StateChange
    where
        S: Healthcheck,
    {
        instances
            .iter_mut()
            .map(|instance| instance.healthcheck(epoch, output_verbosity))
            .collect::<FuturesUnordered<_>>()
            .fold(StateChange::Unchanged, |accumulated, instance| async move {
                accumulated & instance
            })
            .await
    }

    #[inline]
    pub async fn write_out<W>(
        &self,
        mut writer: W,
        global_prepend: &str,
        prepend: &str,
    ) -> Result<WriteOutStatus>
    where
        W: ServiceOutputWriter,
    {
        match self {
            Self::Generic200Ok(instances) => {
                Self::write_out_instances(
                    &mut writer,
                    instances,
                    global_prepend,
                    prepend,
                )
                .await
            }
            Self::Node(instances) => {
                Self::write_out_instances(
                    &mut writer,
                    instances,
                    global_prepend,
                    prepend,
                )
                .await
            }
        }
    }

    async fn write_out_instances<W, C, S>(
        writer: &mut W,
        instances: &[Instance<C, S>],
        global_prepend: &str,
        prepend: &str,
    ) -> Result<WriteOutStatus>
    where
        W: ServiceOutputWriter,
        C: Configuration,
    {
        let mut healthy_instances = 0;

        for prepend in [global_prepend, prepend]
            .iter()
            .copied()
            .filter(|prepend| !prepend.is_empty())
        {
            writer.write_out_prepended(prepend).await?;
        }

        for output in instances.iter().filter_map(|instance| {
            matches!(instance.enabled(), Status::Enabled)
                .then(|| instance.configuration().output())
        }) {
            healthy_instances += 1;

            writer.write_out_entry(output).await?;
        }

        Ok(WriteOutStatus { healthy_instances })
    }
}

impl Service {
    #[inline]
    pub fn healthcheck<'r>(
        &'r mut self,
        epoch: Epoch,
        output_verbosity: OutputVerbosity<ServiceName<'r>>,
    ) -> impl Future<Output = StateChange> + Send + 'r {
        self.instances.healthcheck(epoch, output_verbosity)
    }

    #[inline]
    pub fn write_out<'r, W>(
        &'r self,
        writer: W,
        global_prepend: &'r str,
    ) -> impl Future<Output = Result<WriteOutStatus>> + 'r
    where
        W: ServiceOutputWriter + 'r,
    {
        self.instances
            .write_out(writer, global_prepend, &self.prepend)
    }
}

pub(crate) struct WriteOutStatus {
    pub healthy_instances: usize,
}

pub(crate) trait ServiceOutputWriter {
    fn write_out_prepended<'r>(
        &'r mut self,
        output: &'r str,
    ) -> impl Future<Output = Result<()>> + 'r;

    fn write_out_entry<'r>(
        &'r mut self,
        output: &'r str,
    ) -> impl Future<Output = Result<()>> + 'r;
}

impl<'r, T> ServiceOutputWriter for &'r mut T
where
    T: ServiceOutputWriter,
{
    #[inline]
    fn write_out_prepended<'t>(
        &'t mut self,
        output: &'t str,
    ) -> impl Future<Output = Result<()>> + 't {
        T::write_out_prepended(self, output)
    }

    #[inline]
    fn write_out_entry<'t>(
        &'t mut self,
        output: &'t str,
    ) -> impl Future<Output = Result<()>> + 't {
        T::write_out_entry(self, output)
    }
}

async fn map_and_collect_futures<
    StorageConfiguration,
    Map,
    MapFuture,
    Output,
    Error,
>(
    configuration: configuration::Instances<StorageConfiguration>,
    map: Map,
) -> Result<Vec<Output>, Error>
where
    Map: FnMut((Box<str>, StorageConfiguration)) -> MapFuture,
    MapFuture: Future<Output = Result<Output, Error>>,
{
    configuration
        .into_iter()
        .map(map)
        .collect::<FuturesUnordered<_>>()
        .try_collect()
        .await
}

async fn generic_service(
    prepend: Box<str>,
    configuration: configuration::Instances<
        generic_200_ok::StorageConfiguration,
    >,
) -> Result<Service> {
    map_and_collect_futures(configuration, |(instance_name, configuration)| {
        configuration.create_instance(instance_name)
    })
    .await
    .map(Vec::into_boxed_slice)
    .map(Instances::Generic200Ok)
    .map(|instances| Service { prepend, instances })
}

async fn node_services(
    service_name: String,
    prepend: Box<str>,
    configuration: configuration::Instances<node::StorageConfiguration>,
) -> Result<[(Box<str>, Service); 3]> {
    map_and_collect_futures(configuration, |(instance_name, configuration)| {
        configuration.create_instance(instance_name)
    })
    .await
    .map(|instances| {
        instances.into_iter().fold(
            node::Instances {
                lcd: vec![],
                json_rpc: vec![],
                grpc: vec![],
            },
            |mut instances, group| {
                instances.lcd.push(group.lcd);

                instances.json_rpc.push(group.json_rpc);

                instances.grpc.push(group.grpc);

                instances
            },
        )
    })
    .map(|instances| {
        [
            (service_name.clone(), "_lcd", prepend.clone(), instances.lcd),
            (
                service_name.clone(),
                "_rpc",
                prepend.clone(),
                instances.json_rpc,
            ),
            (service_name, "_grpc", prepend, instances.grpc),
        ]
        .map(|(mut service_name, suffix, prepend, instances)| {
            service_name.push_str(suffix);

            (
                service_name.into_boxed_str(),
                Service {
                    prepend,
                    instances: Instances::Node(instances.into_boxed_slice()),
                },
            )
        })
    })
}

fn collect_services_from_iter<const N: usize>(
    services: &mut Services,
) -> impl FnMut([(Box<str>, Service); N]) -> Result<()> + '_ {
    |iter| {
        IntoIterator::into_iter(iter).try_for_each(|(service_name, service)| {
            match services.entry(service_name) {
                BTreeMapEntry::Vacant(entry) => {
                    entry.insert(service);

                    Ok(())
                }
                BTreeMapEntry::Occupied { .. } => {
                    Err(anyhow!("Collision detected in the services' names!"))
                }
            }
        })
    }
}
