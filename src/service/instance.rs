use std::ops::BitAnd;

use crate::state::Epoch;

#[derive(Clone)]
pub(crate) struct Instance<C, S> {
    instance_name: Box<str>,
    status: Status,
    configuration: C,
    state: S,
}

impl<C, S> Instance<C, S> {
    #[inline]
    pub const fn new(
        instance_name: Box<str>,
        status: Status,
        configuration: C,
        state: S,
    ) -> Self {
        Self {
            instance_name,
            status,
            configuration,
            state,
        }
    }

    #[inline]
    pub const fn enabled(&self) -> Status {
        self.status
    }

    #[inline]
    pub const fn configuration(&self) -> &C {
        &self.configuration
    }
}

impl<C, S> Instance<C, S>
where
    S: Healthcheck,
{
    pub async fn healthcheck(
        &mut self,
        epoch: Epoch,
        output_verbosity: OutputVerbosity<ServiceName<'_>>,
    ) -> StateChange {
        let enabled = self
            .state
            .healthcheck(
                epoch,
                output_verbosity.map(|ServiceName { service_name }| {
                    InstanceName {
                        service_name,
                        instance_name: &self.instance_name,
                    }
                }),
            )
            .await;

        if std::mem::replace(&mut self.status, enabled) != enabled {
            StateChange::Changed
        } else {
            StateChange::Unchanged
        }
    }
}

pub(crate) trait Configuration {
    fn output(&self) -> &str;
}

pub(crate) trait Healthcheck {
    async fn healthcheck(
        &mut self,
        epoch: Epoch,
        output_verbosity: OutputVerbosity<InstanceName<'_>>,
    ) -> Status;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub(crate) enum OutputVerbosity<T> {
    #[default]
    Standard,
    Verbose(T),
}

impl<T> OutputVerbosity<T> {
    pub fn map<F, U>(self, f: F) -> OutputVerbosity<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Self::Standard => OutputVerbosity::Standard,
            Self::Verbose(value) => OutputVerbosity::Verbose(f(value)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ServiceName<'r> {
    pub service_name: &'r str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InstanceName<'r> {
    pub service_name: &'r str,
    pub instance_name: &'r str,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Status {
    Disabled,
    Enabled,
}

impl From<bool> for Status {
    #[inline]
    fn from(value: bool) -> Self {
        if value {
            Self::Enabled
        } else {
            Self::Disabled
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum StateChange {
    Unchanged,
    Changed,
}

impl BitAnd for StateChange {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Self::Changed, _) | (_, Self::Changed) => Self::Changed,
            _ => Self::Unchanged,
        }
    }
}
