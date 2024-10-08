use std::collections::BTreeMap;

use serde::Deserialize;

use crate::{
    serde::deserialize_boxed_string,
    services::{generic_200_ok, node},
};

pub(crate) type Configurations = BTreeMap<Box<str>, Configuration>;

macro_rules! define_configuration {
    ($visibility:vis enum $enum:ident {
        $(
            $(#[$($attributes:tt)+])*
            $variant:ident < $configuration:ty $(,)? >
        ),+ $(,)?
    }) => {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields, rename_all = "snake_case", tag = "type")]
        $visibility enum $enum {
            $(
                $(#[$($attributes)+])*
                $variant {
                    #[serde(default, deserialize_with = "deserialize_boxed_string")]
                    prepend: Box<str>,
                    instances: Instances<$configuration>,
                },
            )+
        }
    };
}

define_configuration! {
    pub(crate) enum Configuration {
        #[serde(rename = "generic_200_ok")]
        Generic200Ok<generic_200_ok::StorageConfiguration>,
        Node<node::StorageConfiguration>,
    }
}

pub(crate) type Instances<C> = BTreeMap<Box<str>, C>;
