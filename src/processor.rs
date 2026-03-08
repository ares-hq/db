use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::{json, Value};
use std::collections::HashMap;

use crate::config::Config;
use crate::first_api::TeamOpr;

pub struct Processor {
    config: Config,
    supabase_client: postgrest::Postgrest,
}

impl Processor {
    pub fn new(config: Config) -> Self {
        let supabase_client = postgrest::Postgrest::new(&config.supabase_url)
            .insert_header("apikey", &config.supabase_key)
            .insert_header("Authorization", format!("Bearer {}", &config.supabase_key));

        Self {
            config,
            supabase_client,
        }
    }

    pub async fn merge_with_database(&self, mut teams: HashMap<i32, TeamOpr>, force_update: bool) -> Result<HashMap<i32, TeamOpr>> {
        let existing = self
            .supabase_client
            .from(&self.config.season_table)
            .select("teamNumber,teamName,sponsors,location,autoOPR,teleOPR,endgameOPR,overallOPR,penalties,autoRank,teleRank,endgameRank,overallRank,penaltyRank,eventsAttended,founded,website")
            .execute()
            .await
            .context("failed to fetch existing teams from database")?
            .text()
            .await
            .context("failed to read response body")?;

        let existing_teams: Vec<Value> = serde_json::from_str(&existing).unwrap_or_default();

        for row in existing_teams {
            let team_number = row["teamNumber"].as_i64().unwrap_or(0) as i32;
            let existing_opr = row["overallOPR"].as_f64().unwrap_or(-100.0);

            if let Some(team) = teams.get_mut(&team_number) {
                let events_str = row["eventsAttended"].as_str().unwrap_or("[]");
                let existing_events: Vec<String> = serde_json::from_str(events_str).unwrap_or_default();
                let mut merged: Vec<String> = existing_events
                    .into_iter()
                    .chain(team.events_attended.clone())
                    .collect::<std::collections::HashSet<_>>()
                    .into_iter()
                    .collect();
                merged.sort();
                team.events_attended = merged;

                if team.founded.is_none() {
                    team.founded = row["founded"].as_i64().map(|v| v as i32);
                }
                if team.website.is_none() {
                    team.website = row["website"].as_str().map(|s| s.to_owned());
                }

                if !force_update && team.overall_opr <= existing_opr {
                    team.team_name = row["teamName"].as_str().unwrap_or("Unknown").to_owned();
                    team.sponsors = row["sponsors"].as_str().unwrap_or("Unknown").to_owned();
                    team.location = row["location"].as_str().unwrap_or("Unknown").to_owned();
                    team.auto_opr = row["autoOPR"].as_f64().unwrap_or(0.0);
                    team.tele_opr = row["teleOPR"].as_f64().unwrap_or(0.0);
                    team.endgame_opr = row["endgameOPR"].as_f64().unwrap_or(0.0);
                    team.overall_opr = existing_opr;
                    team.penalties = row["penalties"].as_f64().unwrap_or(0.0);
                    team.auto_rank = row["autoRank"].as_i64().map(|v| v as i32);
                    team.tele_rank = row["teleRank"].as_i64().map(|v| v as i32);
                    team.endgame_rank = row["endgameRank"].as_i64().map(|v| v as i32);
                    team.overall_rank = row["overallRank"].as_i64().map(|v| v as i32);
                    team.penalty_rank = row["penaltyRank"].as_i64().map(|v| v as i32);
                }
            }
        }

        Ok(teams)
    }

    pub fn update_rankings(&self, teams: &mut HashMap<i32, TeamOpr>) {
        let mut team_list: Vec<&mut TeamOpr> = teams.values_mut().collect();

        Self::assign_rank(&mut team_list, |t| t.overall_opr, |t, r| t.overall_rank = Some(r), true);
        Self::assign_rank(&mut team_list, |t| t.auto_opr, |t, r| t.auto_rank = Some(r), true);
        Self::assign_rank(&mut team_list, |t| t.tele_opr, |t, r| t.tele_rank = Some(r), true);
        Self::assign_rank(&mut team_list, |t| t.endgame_opr, |t, r| t.endgame_rank = Some(r), true);
        Self::assign_rank(&mut team_list, |t| t.penalties, |t, r| t.penalty_rank = Some(r), false);
    }

    fn assign_rank<F, S>(teams: &mut [&mut TeamOpr], score_fn: F, set_rank: S, reverse: bool)
    where
        F: Fn(&TeamOpr) -> f64,
        S: Fn(&mut TeamOpr, i32),
    {
        teams.sort_by(|a, b| {
            let sa = score_fn(a);
            let sb = score_fn(b);
            if reverse {
                sb.partial_cmp(&sa).unwrap_or(std::cmp::Ordering::Equal)
            } else {
                sa.partial_cmp(&sb).unwrap_or(std::cmp::Ordering::Equal)
            }
        });

        let mut current_rank = 1;
        for i in 0..teams.len() {
            if i > 0 && (score_fn(teams[i]) - score_fn(teams[i - 1])).abs() > 1e-6 {
                current_rank = (i + 1) as i32;
            }
            set_rank(teams[i], current_rank);
        }
    }

    pub async fn upsert_to_database(&self, teams: &HashMap<i32, TeamOpr>) -> Result<()> {
        let now = Utc::now();
        let timestamp = now.format("%Y-%m-%dT%H:%M:%S%.3f").to_string();

        let rows: Vec<Value> = teams
            .values()
            .map(|team| {
                json!({
                    "teamNumber": team.team_number,
                    "teamName": team.team_name,
                    "sponsors": team.sponsors,
                    "location": team.location,
                    "autoOPR": team.auto_opr,
                    "teleOPR": team.tele_opr,
                    "endgameOPR": team.endgame_opr,
                    "overallOPR": team.overall_opr,
                    "penalties": team.penalties,
                    "autoRank": team.auto_rank,
                    "teleRank": team.tele_rank,
                    "endgameRank": team.endgame_rank,
                    "overallRank": team.overall_rank,
                    "penaltyRank": team.penalty_rank,
                    "profileUpdate": timestamp,
                    "founded": team.founded,
                    "website": team.website,
                    "eventsAttended": team.events_attended,
                })
            })
            .collect();

        if !rows.is_empty() {
            let body = serde_json::to_string(&rows)?;
            self.supabase_client
                .from(&self.config.season_table)
                .upsert(body)
                .execute()
                .await
                .context("failed to upsert teams")?;

            tracing::info!("✅ Upserted {} teams to database", rows.len());
        }

        Ok(())
    }
}
