use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use tokio::{
    select,
    signal::unix::{signal, SignalKind},
    time::{interval, MissedTickBehavior},
};
use tracing::{info, Level};

use crate::state::State;

mod configuration;
mod http_client;
mod serde;
mod service;
mod services;
mod state;
mod writer;

#[derive(Parser)]
struct Args {
    #[clap(
        short = 'c',
        long,
        visible_alias = "static-config",
        visible_alias = "static-configuration"
    )]
    r#static: PathBuf,
    #[clap(
        short,
        long,
        visible_alias = "services-config",
        visible_alias = "services-configuration"
    )]
    services: PathBuf,
    #[clap(
        short,
        long,
        visible_alias = "output-config",
        visible_alias = "output-configuration"
    )]
    output: PathBuf,
}

#[tokio::main]
async fn main() -> Result<()> {
    let Args {
        r#static,
        services,
        output,
    } = Parser::parse();

    initialize_logging();

    let mut state = State::load(&r#static, &services).await?;

    let mut interval = {
        let mut interval = interval(state.refresh_period());

        interval.set_missed_tick_behavior(MissedTickBehavior::Skip);

        interval
    };

    let mut signal = signal(SignalKind::hangup())?;

    let mut forced = true;

    loop {
        select! {
            biased;
            _ = interval.tick() => {},
            _ = signal.recv() => {
                info!("\"SIGHUP\" received. Configuration reload requested.");

                state = State::load(
                    &r#static,
                    &services,
                )
                .await?;

                forced = true;
            },
        }

        state.output_configuration(&output, forced).await?;

        forced = false;
    }
}

fn initialize_logging() {
    tracing_subscriber::fmt::fmt()
        .compact()
        .with_ansi(true)
        .with_file(false)
        .with_level(true)
        .with_line_number(false)
        .with_target(false)
        .with_thread_ids(false)
        .with_max_level(
            if cfg!(debug_assertions)
                || std::env::var_os("DEBUG_LOG")
                    .map_or(false, |value| value == "1")
            {
                Level::DEBUG
            } else {
                Level::INFO
            },
        )
        .with_thread_names(false)
        .with_writer(std::io::stdout)
        .init()
}
