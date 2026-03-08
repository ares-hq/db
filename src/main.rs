mod api_client;
mod api_params;
mod config;
mod first_api;
mod processor;
mod utils;
mod year_adapters;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

use crate::api_client::ApiClient;
use crate::config::Config;
use crate::first_api::FirstApi;
use crate::processor::Processor;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    tracing::info!("Starting ARES Database Pipeline");

    let config = Config::from_env()?;
    tracing::info!("Loaded configuration");

    let api_client = ApiClient::new(
        "https://ftc-api.firstinspires.org/v2.0",
        config.first_username.clone(),
        config.first_password.clone(),
    )?;
    tracing::info!("Initialized FIRST API client");

    let first_api = FirstApi::new(api_client);
    let processor = Processor::new(config.clone());

    let year = 2025;
    let all_events = std::env::args().any(|arg| arg == "--all-events");
    let force_update = std::env::args().any(|arg| arg == "--force-update");

    tracing::info!("Fetching season data for year {}", year);
    let mut teams = first_api.fetch_season_data(year, all_events).await?;
    tracing::info!("Fetched data for {} teams", teams.len());

    tracing::info!("Merging with existing database");
    teams = processor.merge_with_database(teams, force_update).await?;
    tracing::info!("Merge complete");

    tracing::info!("Updating rankings");
    processor.update_rankings(&mut teams);
    tracing::info!("Rankings updated");

    tracing::info!("Upserting to database");
    processor.upsert_to_database(&teams).await?;
    tracing::info!("Database updated successfully");

    tracing::info!("Pipeline complete");
    Ok(())
}
