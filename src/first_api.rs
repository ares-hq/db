use crate::level::Level;
use anyhow::{Context, Result, anyhow};
use futures::future::join_all;
use model::prelude::{Alliance, Event, Match, Team};
use serde_json::Value;
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use tokio::sync::Semaphore;

use crate::api_client::ApiClient;
use crate::api_params::ApiParams;
use crate::matches::MatchRow;
use crate::opr::{self, Opr};
use crate::year_adapters::adapter_for_year;

pub struct FirstApi {
    client: ApiClient,
}

impl FirstApi {
    pub fn new(client: ApiClient) -> Self {
        Self { client }
    }

    pub async fn fetch_season_data(
        &self,
        year: i32,
        all_events: bool,
    ) -> Result<(HashMap<u32, Team>, Vec<MatchRow>)> {
        let events = if all_events {
            self.get_all_events(year).await?
        } else {
            self.get_future_events(year).await?
        };

        tracing::info!("Processing {} events for season {}", events.len(), year);

        let mut roster = self.fetch_team_roster(year).await?;
        tracing::info!(teams = roster.len(), "Fetched season team roster");

        // Solve independently, merge later in one owner — no lock needed.
        let sem = Semaphore::new(16);
        let tasks: Vec<_> = events
            .iter()
            .map(|event_code| {
                let client = self.client.clone();
                let sem = &sem;
                async move {
                    let _permit = sem.acquire().await.ok()?;
                    match Self::solve_event(&client, event_code, year).await {
                        Ok(outcome) => Some((event_code.as_str(), outcome)),
                        Err(err) => {
                            tracing::warn!(event = %event_code, error = %err, "Skipping event");
                            None
                        }
                    }
                }
            })
            .collect();

        let outcomes: Vec<(&str, (Opr, Vec<MatchRow>))> =
            join_all(tasks).await.into_iter().flatten().collect();

        let mut matches = Vec::new();
        let mut solved: Vec<(&str, Opr)> = Vec::with_capacity(outcomes.len());
        for (code, (opr, rows)) in outcomes {
            matches.extend(rows);
            solved.push((code, opr));
        }

        let added = self.fetch_missing_teams(year, &solved, &mut roster).await;
        if added > 0 {
            tracing::info!(
                teams = added,
                "Fetched teams absent from the roster listing"
            );
        }

        Ok((merge_events(&roster, &solved), matches))
    }

    /// Teams that played but missed the roster; rare, so sequential. Returns count added.
    async fn fetch_missing_teams(
        &self,
        year: i32,
        solved: &[(&str, Opr)],
        roster: &mut HashMap<u32, Team>,
    ) -> usize {
        let mut missing: Vec<u32> = solved
            .iter()
            .flat_map(|(_, opr)| opr.teams.iter().copied())
            .filter(|number| !roster.contains_key(number))
            .collect();
        missing.sort_unstable();
        missing.dedup();

        let mut added = 0;
        for number in missing {
            match Self::fetch_team_info(&self.client, number as i32, year).await {
                Ok(team) => {
                    roster.insert(number, team);
                    added += 1;
                }
                Err(err) => {
                    tracing::warn!(team = number, error = %err, "Skipping unknown team");
                }
            }
        }
        added
    }

    /// Whole season roster by team number; paged 500 at a time, not per-team.
    async fn fetch_team_roster(&self, year: i32) -> Result<HashMap<u32, Team>> {
        let first = self.fetch_team_page(year, 1).await?;
        let page_total = first["pageTotal"].as_i64().unwrap_or(1).max(1);

        let mut pages = vec![first];
        let sem = Semaphore::new(8);
        let rest = join_all((2..=page_total).map(|page| {
            let sem = &sem;
            async move {
                let _permit = sem.acquire().await.ok()?;
                match self.fetch_team_page(year, page).await {
                    Ok(v) => Some(v),
                    Err(err) => {
                        tracing::warn!(page, error = %err, "Skipping team roster page");
                        None
                    }
                }
            }
        }))
        .await;
        pages.extend(rest.into_iter().flatten());

        Ok(pages
            .iter()
            .filter_map(|p| p["teams"].as_array())
            .flatten()
            .filter_map(team_from_json)
            .map(|team| (team.number, team))
            .collect())
    }

    async fn fetch_team_page(&self, year: i32, page: i64) -> Result<Value> {
        let params = ApiParams::new(vec![year.to_string(), "teams".to_owned()])
            .with_query("page", page.to_string());
        self.client.get_json(&params).await
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

    /// One event's matches and its OPR.
    async fn solve_event(
        client: &ApiClient,
        event_code: &str,
        year: i32,
    ) -> Result<(Opr, Vec<MatchRow>)> {
        let (matches_raw, scores_raw) = tokio::try_join!(
            Self::fetch_event_matches(client, event_code, year),
            Self::fetch_event_scores(client, event_code, year),
        )?;

        // Scores cover only quals; match number joins them, never array position.
        let scores_by_number: HashMap<i64, &Value> = scores_raw
            .iter()
            .filter_map(|s| s["matchNumber"].as_i64().map(|n| (n, s)))
            .collect();

        let adapter = adapter_for_year(year);
        let mut event = Event::new(event_code);

        for match_obj in matches_raw.iter().filter(|m| {
            Level::from_api(m["tournamentLevel"].as_str().unwrap_or("")) == Level::Qualification
        }) {
            let Some(score_obj) = match_obj["matchNumber"]
                .as_i64()
                .and_then(|n| scores_by_number.get(&n))
            else {
                continue;
            };

            let (red_endgame, blue_endgame) = adapter.endgame_points(score_obj);
            let (red_penalty, blue_penalty) = adapter.penalties(score_obj);

            let (red_teams, blue_teams) = alliance_teams(match_obj);
            let points = |key: &str| match_obj[key].as_f64().unwrap_or(0.0);

            // Teleop = final - auto - opponent fouls.
            event.add_match(Match::new(
                Alliance {
                    teams: red_teams,
                    auto: points("scoreRedAuto"),
                    teleop: points("scoreRedFinal")
                        - points("scoreRedAuto")
                        - points("scoreBlueFoul"),
                    endgame: red_endgame as f64,
                    penalties: red_penalty as f64,
                },
                Alliance {
                    teams: blue_teams,
                    auto: points("scoreBlueAuto"),
                    teleop: points("scoreBlueFinal")
                        - points("scoreBlueAuto")
                        - points("scoreRedFoul"),
                    endgame: blue_endgame as f64,
                    penalties: blue_penalty as f64,
                },
            ));
        }

        if event.is_empty() {
            return Err(anyhow!("no scored qualification matches"));
        }

        // Match log keeps every level, not just quals.
        let matches = matches_raw
            .iter()
            .flat_map(|m| MatchRow::from_match(event_code, m))
            .collect();

        Ok((opr::solve(&event)?, matches))
    }

    async fn fetch_event_matches(
        client: &ApiClient,
        event_code: &str,
        year: i32,
    ) -> Result<Vec<Value>> {
        let params = ApiParams::new(vec![
            year.to_string(),
            "matches".to_owned(),
            event_code.to_owned(),
        ]);
        let resp = client.get_json(&params).await?;
        Ok(resp["matches"].as_array().cloned().unwrap_or_default())
    }

    async fn fetch_event_scores(
        client: &ApiClient,
        event_code: &str,
        year: i32,
    ) -> Result<Vec<Value>> {
        let params = ApiParams::new(vec![
            year.to_string(),
            "scores".to_owned(),
            event_code.to_owned(),
            "qual".to_owned(),
        ]);
        let resp = client.get_json(&params).await?;
        Ok(resp["matchScores"].as_array().cloned().unwrap_or_default())
    }

    async fn fetch_team_info(client: &ApiClient, team_number: i32, year: i32) -> Result<Team> {
        let params = ApiParams::new(vec![year.to_string(), "teams".to_owned()])
            .with_query("teamNumber", team_number.to_string());
        let resp = client.get_json(&params).await?;
        team_from_json(&resp["teams"][0])
            .with_context(|| format!("team {team_number} not found for season {year}"))
    }
}

/// One record per team: figures from its strongest event by OPR, all events recorded.
fn merge_events(roster: &HashMap<u32, Team>, solved: &[(&str, Opr)]) -> HashMap<u32, Team> {
    let mut all_teams: HashMap<u32, Team> = HashMap::new();

    for (event_code, opr) in solved {
        for (number, [auto, teleop, endgame, penalties]) in opr.iter() {
            let Some(base) = roster.get(&number) else {
                continue;
            };

            let mut team = base.clone();
            team.auto = auto;
            team.teleop = teleop;
            team.endgame = endgame;
            team.penalties = penalties;
            team.recompute_overall();
            team.add_events([(*event_code).to_owned()]);
            team.update_last_match();

            match all_teams.entry(number) {
                Entry::Occupied(mut slot) => {
                    let existing = slot.get_mut();
                    if team.overall > existing.overall {
                        let events = std::mem::take(&mut existing.events_attended);
                        *existing = team;
                        existing.add_events(events);
                    } else {
                        existing.add_events(team.events_attended);
                    }
                }
                Entry::Vacant(slot) => {
                    slot.insert(team);
                }
            }
        }
    }

    all_teams
}

/// One `teams` response entry, shared by roster pages and lookups.
fn team_from_json(team: &Value) -> Option<Team> {
    let number = team["teamNumber"].as_i64()?;

    Some(Team {
        number: u32::try_from(number).ok()?,
        name: team["nameShort"].as_str().unwrap_or("Unknown").to_owned(),
        sponsors: team["nameFull"]
            .as_str()
            .unwrap_or("Unknown")
            .replace("/", ", ")
            .replace("&", ", "),
        location: format!(
            "{}, {}, {}",
            team["city"].as_str().unwrap_or("Unknown"),
            team["stateProv"].as_str().unwrap_or("Unknown"),
            team["country"].as_str().unwrap_or("Unknown")
        ),
        founded: team["rookieYear"]
            .as_i64()
            .and_then(|v| u16::try_from(v).ok()),
        website: team["website"].as_str().map(|s| s.to_owned()),
        ..Default::default()
    })
}

/// Red/blue split by station name (API order is not guaranteed); an absent team stays `0`.
fn alliance_teams(match_obj: &Value) -> ([u32; 2], [u32; 2]) {
    let (mut red, mut blue) = ([0u32; 2], [0u32; 2]);

    let Some(teams) = match_obj["teams"].as_array() else {
        return (red, blue);
    };

    for entry in teams {
        if !entry["onField"].as_bool().unwrap_or(true) {
            continue;
        }
        let Some(number) = entry["teamNumber"]
            .as_i64()
            .and_then(|n| u32::try_from(n).ok())
        else {
            continue;
        };
        let station = entry["station"].as_str().unwrap_or("");
        let slot = match station.chars().last() {
            Some('1') => 0,
            Some('2') => 1,
            _ => continue,
        };

        if station.starts_with("Red") {
            red[slot] = number;
        } else if station.starts_with("Blue") {
            blue[slot] = number;
        }
    }

    (red, blue)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn station(number: u32, station: &str, on_field: bool) -> Value {
        json!({ "teamNumber": number, "station": station, "onField": on_field })
    }

    #[test]
    fn teams_are_placed_by_station_not_by_order() {
        let m = json!({ "teams": [
            station(4, "Blue2", true),
            station(1, "Red1", true),
            station(3, "Blue1", true),
            station(2, "Red2", true),
        ]});

        assert_eq!(alliance_teams(&m), ([1, 2], [3, 4]));
    }

    #[test]
    fn an_off_field_team_leaves_its_slot_empty() {
        let m = json!({ "teams": [
            station(1, "Red1", true),
            station(2, "Red2", false),
            station(3, "Blue1", true),
            station(4, "Blue2", true),
        ]});

        // Team 2 is absent; team 1 must not slide into its place.
        assert_eq!(alliance_teams(&m), ([1, 0], [3, 4]));
    }

    #[test]
    fn a_missing_team_list_yields_empty_alliances() {
        assert_eq!(alliance_teams(&json!({})), ([0, 0], [0, 0]));
    }

    #[test]
    fn unknown_stations_are_ignored() {
        let m = json!({ "teams": [
            station(1, "Red1", true),
            station(9, "Green1", true),
            station(7, "", true),
        ]});

        assert_eq!(alliance_teams(&m), ([1, 0], [0, 0]));
    }
}

#[cfg(test)]
mod merge_tests {
    use super::*;
    use ndarray::Array1;

    fn roster(numbers: &[u32]) -> HashMap<u32, Team> {
        numbers
            .iter()
            .map(|&n| (n, Team::new(n, format!("Team {n}"))))
            .collect()
    }

    fn opr(teams: &[u32], scores: &[f64]) -> Opr {
        let col = || Array1::from_iter(scores.iter().copied());
        Opr {
            teams: teams.to_vec(),
            auto: col(),
            teleop: col(),
            endgame: col(),
            penalties: col(),
        }
    }

    #[test]
    fn scores_land_on_the_roster_team() {
        let merged = merge_events(&roster(&[1]), &[("TXHOU", opr(&[1], &[12.5]))]);

        let team = &merged[&1];
        assert_eq!(team.name, "Team 1");
        assert_eq!(team.auto, 12.5);
        assert_eq!(team.penalties, 12.5);
        assert_eq!(team.overall, 25.0, "overall is auto + teleop only");
        assert_eq!(team.events_attended, vec!["TXHOU"]);
    }

    #[test]
    fn the_strongest_event_supplies_the_season_figures() {
        let merged = merge_events(
            &roster(&[1]),
            &[
                ("WEAK", opr(&[1], &[5.0])),
                ("STRONG", opr(&[1], &[50.0])),
                ("MID", opr(&[1], &[20.0])),
            ],
        );

        assert_eq!(merged[&1].auto, 50.0);
    }

    #[test]
    fn every_event_is_recorded_whichever_one_wins() {
        let merged = merge_events(
            &roster(&[1]),
            &[("WEAK", opr(&[1], &[5.0])), ("STRONG", opr(&[1], &[50.0]))],
        );

        assert_eq!(merged[&1].events_attended, vec!["STRONG", "WEAK"]);
    }

    #[test]
    fn events_survive_when_a_later_event_is_weaker() {
        // Both the swap path and the keep path must preserve history.
        let merged = merge_events(
            &roster(&[1]),
            &[("STRONG", opr(&[1], &[50.0])), ("WEAK", opr(&[1], &[5.0]))],
        );

        assert_eq!(merged[&1].auto, 50.0);
        assert_eq!(merged[&1].events_attended, vec!["STRONG", "WEAK"]);
    }

    #[test]
    fn a_team_missing_from_the_roster_is_skipped() {
        let merged = merge_events(&roster(&[1]), &[("TXHOU", opr(&[1, 99], &[10.0, 20.0]))]);

        assert!(merged.contains_key(&1));
        assert!(
            !merged.contains_key(&99),
            "invented a team with no metadata"
        );
    }

    #[test]
    fn negative_opr_still_merges() {
        let merged = merge_events(&roster(&[1]), &[("TXHOU", opr(&[1], &[-8.25]))]);
        assert_eq!(merged[&1].auto, -8.25);
        assert_eq!(merged[&1].overall, -16.5);
    }

    #[test]
    fn merging_marks_the_team_as_played() {
        let merged = merge_events(&roster(&[1]), &[("TXHOU", opr(&[1], &[10.0]))]);
        assert!(merged[&1].has_played());
    }
}
