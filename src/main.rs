mod config;
mod first_api;
mod matches;
mod opr;
mod processor;
mod utils;
mod year_adapters;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;

use crate::config::Config;
use crate::first_api::FirstApi;
use crate::processor::Processor;
use model::first::{self, ApiClient};
use model::prelude::current_season;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

/// Fetches a season from the FIRST API, solves per-team OPR, ranks every team and writes it
/// to Supabase.
#[derive(Parser)]
struct Args {
    /// Season; defaults to the current one, rolling over in August.
    #[arg(long)]
    year: Option<i32>,
    /// Whole schedule, not just the last week onward. Implied for past seasons.
    #[arg(long)]
    all_events: bool,
    /// Overwrite stored figures even when the stored event was stronger.
    #[arg(long)]
    force_update: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .init();

    tracing::info!("Starting ARES Database Pipeline");

    let config = Config::from_env()?;
    tracing::info!("Loaded configuration");

    let api_client = ApiClient::new(
        first::BASE_URL,
        config.first_username.clone(),
        config.first_password.clone(),
    )?;
    tracing::info!("Initialized FIRST API client");

    let this_season = {
        use chrono::Datelike;
        let now = chrono::Utc::now();
        current_season(now.year(), now.month())
    };
    let year = args.year.unwrap_or(this_season);
    let all_events = args.all_events || year < this_season;

    let first_api = FirstApi::new(api_client);
    let processor = Processor::new(config, year);

    tracing::info!("Fetching season data for year {}", year);
    let (teams, matches) = first_api.fetch_season_data(year, all_events).await?;
    tracing::info!(
        "Fetched {} teams, {} match rows",
        teams.len(),
        matches.len()
    );

    if teams.is_empty() && matches.is_empty() {
        tracing::info!("Nothing to update");
        return Ok(());
    }

    tracing::info!("Merging with the stored season");
    let stored = processor.stored().await?;
    let mut season = Processor::merge(&stored, teams, args.force_update);
    tracing::info!(teams = season.len(), "Merge complete");

    Processor::rank(&mut season);
    tracing::info!("Rankings updated");

    processor.upsert_changed(&stored, season).await?;
    processor.upsert_matches(&matches).await?;
    tracing::info!("Pipeline complete");
    Ok(())
}
