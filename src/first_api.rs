use anyhow::{Context, Result};
use futures::future::join_all;
use serde_json::Value;
use std::collections::HashMap;
use tokio::sync::Semaphore;

use crate::api_client::ApiClient;
use crate::api_params::ApiParams;
use crate::utils::matrix_math::solve_metrics;
use crate::utils::team_builder::{MatchData, MatchTeam, MatrixBuilder};
use crate::year_adapters::adapter_for_year;

#[derive(Debug, Clone, serde::Serialize)]
pub struct TeamOpr {
    pub team_number: i32,
    pub team_name: String,
    pub sponsors: String,
    pub location: String,
    pub auto_opr: f64,
    pub tele_opr: f64,
    pub endgame_opr: f64,
    pub overall_opr: f64,
    pub penalties: f64,
    pub auto_rank: Option<i32>,
    pub tele_rank: Option<i32>,
    pub endgame_rank: Option<i32>,
    pub overall_rank: Option<i32>,
    pub penalty_rank: Option<i32>,
    pub events_attended: Vec<String>,
    pub founded: Option<i32>,
    pub website: Option<String>,
}

pub struct FirstApi {
    client: ApiClient,
}

impl FirstApi {
    pub fn new(client: ApiClient) -> Self {
        Self { client }
    }

    pub async fn fetch_season_data(&self, year: i32, all_events: bool) -> Result<HashMap<i32, TeamOpr>> {
        let events = if all_events {
            self.get_all_events(year).await?
        } else {
            self.get_future_events(year).await?
        };

        tracing::info!("Processing {} events for season {}", events.len(), year);

        let mut all_teams: HashMap<i32, TeamOpr> = HashMap::new();
        let sem = Semaphore::new(16);

        let tasks: Vec<_> = events
            .iter()
            .map(|event_code| {
                let event_code = event_code.clone();
                let client = self.client.clone();
                let sem = &sem;
                async move {
                    let _permit = sem.acquire().await.ok()?;
                    Self::process_event(&client, &event_code, year).await.ok()
                }
            })
            .collect();

        let results = join_all(tasks).await;

        for maybe_event_teams in results.into_iter().flatten() {
            for (team_num, team_opr) in maybe_event_teams {
                all_teams
                    .entry(team_num)
                    .and_modify(|existing| {
                        if team_opr.overall_opr > existing.overall_opr {
                            *existing = team_opr.clone();
                        }
                    })
                    .or_insert(team_opr);
            }
        }

        Ok(all_teams)
    }

    async fn get_all_events(&self, year: i32) -> Result<Vec<String>> {
        let params = ApiParams::new(vec![year.to_string(), "events".to_owned()]);
        let resp = self.client.get_json(&params).await?;
        let events = resp["events"]
            .as_array()
            .context("events not array")?
            .iter()
            .filter_map(|e| e["code"].as_str().map(|s| s.to_owned()))
            .collect();
        Ok(events)
    }

    async fn get_future_events(&self, year: i32) -> Result<Vec<String>> {
        let params = ApiParams::new(vec![year.to_string(), "events".to_owned()]);
        let resp = self.client.get_json(&params).await?;
        let today = chrono::Utc::now().date_naive();
        let cutoff = today - chrono::Duration::days(7);

        let events = resp["events"]
            .as_array()
            .context("events not array")?
            .iter()
            .filter(|e| {
                e["dateStart"]
                    .as_str()
                    .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
                    .map(|date| date >= cutoff)
                    .unwrap_or(false)
            })
            .filter_map(|e| e["code"].as_str().map(|s| s.to_owned()))
            .collect();
        Ok(events)
    }

    async fn process_event(client: &ApiClient, event_code: &str, year: i32) -> Result<HashMap<i32, TeamOpr>> {
        let matches_raw = Self::fetch_event_matches(client, event_code, year).await?;
        let scores_raw = Self::fetch_event_scores(client, event_code, year).await?;

        let adapter = adapter_for_year(year);
        let mut match_data = vec![];

        for (match_obj, score_obj) in matches_raw.iter().zip(scores_raw.iter()) {
            let (red_endgame, blue_endgame) = adapter.endgame_points(score_obj);
            let (red_penalty, blue_penalty) = adapter.penalties(score_obj);

            let teams: Vec<MatchTeam> = match_obj["teams"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .map(|t| MatchTeam {
                    team_number: t["teamNumber"].as_i64().unwrap_or(0) as i32,
                    station: t["station"].as_str().unwrap_or("").to_owned(),
                    on_field: Some(t["onField"].as_bool().unwrap_or(true)),
                })
                .collect();

            match_data.push(MatchData {
                teams,
                description: match_obj["description"].as_str().unwrap_or("").to_owned(),
                actual_start_time: match_obj["actualStartTime"]
                    .as_str()
                    .unwrap_or("2025-01-01")
                    .to_owned(),
                score_red_final: match_obj["scoreRedFinal"].as_i64().map(|v| v as i32),
                score_red_auto: match_obj["scoreRedAuto"].as_i64().map(|v| v as i32),
                score_blue_foul: match_obj["scoreBlueFoul"].as_i64().map(|v| v as i32),
                score_blue_final: match_obj["scoreBlueFinal"].as_i64().map(|v| v as i32),
                score_blue_auto: match_obj["scoreBlueAuto"].as_i64().map(|v| v as i32),
                score_red_foul: match_obj["scoreRedFoul"].as_i64().map(|v| v as i32),
                score_red_endgame: Some(red_endgame),
                score_blue_endgame: Some(blue_endgame),
                penalty_points_red: Some(red_penalty),
                penalty_points_blue: Some(blue_penalty),
            });
        }

        if match_data.is_empty() {
            return Ok(HashMap::new());
        }

        let builder = MatrixBuilder::new(match_data);
        let solution = solve_metrics(
            &builder.binary_matrix,
            &builder.auto_matrix,
            &builder.tele_matrix,
            &builder.endgame_matrix,
            &builder.penalties_matrix,
        )?;

        let mut teams_map = HashMap::new();
        for (idx, &team_num) in builder.teams.iter().enumerate() {
            let team_info = Self::fetch_team_info(client, team_num, year).await?;
            teams_map.insert(
                team_num,
                TeamOpr {
                    team_number: team_num,
                    team_name: team_info.0,
                    sponsors: team_info.1,
                    location: team_info.2,
                    auto_opr: solution.auto[idx],
                    tele_opr: solution.tele[idx],
                    endgame_opr: solution.endgame[idx],
                    overall_opr: solution.auto[idx] + solution.tele[idx],
                    penalties: solution.penalties[idx],
                    auto_rank: None,
                    tele_rank: None,
                    endgame_rank: None,
                    overall_rank: None,
                    penalty_rank: None,
                    events_attended: vec![event_code.to_owned()],
                    founded: team_info.3,
                    website: team_info.4,
                },
            );
        }

        Ok(teams_map)
    }

    async fn fetch_event_matches(client: &ApiClient, event_code: &str, year: i32) -> Result<Vec<Value>> {
        let params = ApiParams::new(vec![
            year.to_string(),
            "matches".to_owned(),
            event_code.to_owned(),
        ]);
        let resp = client.get_json(&params).await?;
        Ok(resp["matches"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    async fn fetch_event_scores(client: &ApiClient, event_code: &str, year: i32) -> Result<Vec<Value>> {
        let params = ApiParams::new(vec![
            year.to_string(),
            "scores".to_owned(),
            event_code.to_owned(),
            "qual".to_owned(),
        ]);
        let resp = client.get_json(&params).await?;
        Ok(resp["matchScores"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    async fn fetch_team_info(
        client: &ApiClient,
        team_number: i32,
        year: i32,
    ) -> Result<(String, String, String, Option<i32>, Option<String>)> {
        let params = ApiParams::new(vec![year.to_string(), "teams".to_owned()])
            .with_query("teamNumber", team_number.to_string());
        let resp = client.get_json(&params).await?;
        let team = &resp["teams"][0];

        let name = team["nameShort"].as_str().unwrap_or("Unknown").to_owned();
        let sponsors = team["nameFull"]
            .as_str()
            .unwrap_or("Unknown")
            .replace("/", ", ")
            .replace("&", ", ");
        let location = format!(
            "{}, {}, {}",
            team["city"].as_str().unwrap_or("Unknown"),
            team["stateProv"].as_str().unwrap_or("Unknown"),
            team["country"].as_str().unwrap_or("Unknown")
        );
        let founded = team["rookieYear"].as_i64().map(|v| v as i32);
        let website = team["website"].as_str().map(|s| s.to_owned());

        Ok((name, sponsors, location, founded, website))
    }
}
