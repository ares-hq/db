use anyhow::{Result, anyhow};
use futures::future::join_all;
use model::prelude::{Alliance, Event, Match, Team};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use tokio::sync::Semaphore;

use crate::api_client::ApiClient;
use crate::api_params::ApiParams;
use crate::api_types::{
    EventsResponse, MatchInfo, MatchScore, MatchesResponse, ScoresResponse, TeamInfo, TeamsResponse,
};
use crate::level::Level;
use crate::matches::MatchRow;
use crate::opr::{self, Opr};
use crate::stations::alliance_teams;
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
        let events = self.get_events(year, all_events).await?;

        tracing::info!("Processing {} events for season {}", events.len(), year);

        let mut roster = self.fetch_team_roster(year).await?;
        tracing::info!(teams = roster.len(), "Fetched season team roster");

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
            match Self::fetch_team_info(&self.client, number, year).await {
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

    /// Paged 500 at a time.
    async fn fetch_team_roster(&self, year: i32) -> Result<HashMap<u32, Team>> {
        let first = self.fetch_team_page(year, 1).await?;
        let page_total = first.page_total.unwrap_or(1).max(1);

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
            .flat_map(|p| p.teams.iter())
            .filter_map(team_from_info)
            .map(|team| (team.number, team))
            .collect())
    }

    async fn fetch_team_page(&self, year: i32, page: i64) -> Result<TeamsResponse> {
        let params = ApiParams::new(vec![year.to_string(), "teams".to_owned()])
            .with_query("page", page.to_string());
        self.client.get(&params).await
    }

    async fn get_events(&self, year: i32, all_events: bool) -> Result<Vec<String>> {
        let params = ApiParams::new(vec![year.to_string(), "events".to_owned()]);
        let resp: EventsResponse = self.client.get(&params).await?;

        let cutoff = chrono::Utc::now().date_naive() - chrono::Duration::days(7);
        Ok(resp
            .events
            .into_iter()
            .filter(|e| all_events || started_on_or_after(e.date_start.as_deref(), cutoff))
            .filter_map(|e| e.code)
            .collect())
    }

    async fn solve_event(
        client: &ApiClient,
        event_code: &str,
        year: i32,
    ) -> Result<(Opr, Vec<MatchRow>)> {
        let (matches_raw, scores_raw) = tokio::try_join!(
            Self::fetch_event_matches(client, event_code, year),
            Self::fetch_event_scores(client, event_code, year),
        )?;

        let scores_by_number: HashMap<i64, &MatchScore> = scores_raw
            .iter()
            .filter_map(|s| s.match_number.map(|n| (n, s)))
            .collect();

        let adapter = adapter_for_year(year);
        let mut event = Event::new(event_code);

        for match_info in matches_raw
            .iter()
            .filter(|m| m.tournament_level == Level::Qualification)
        {
            let Some(score) = scores_by_number.get(&match_info.match_number) else {
                continue;
            };

            let (red_endgame, blue_endgame) = adapter.endgame_points(score);
            let (red_penalty, blue_penalty) = adapter.penalties(score);
            let (red_teams, blue_teams) = alliance_teams(&match_info.teams);

            // The final score carries the opponent's fouls; its teleop share carries endgame.
            event.add_match(Match::new(
                Alliance {
                    teams: red_teams,
                    auto: match_info.score_red_auto as f64,
                    teleop: (match_info.score_red_final
                        - match_info.score_red_auto
                        - match_info.score_blue_foul
                        - red_endgame as i64) as f64,
                    endgame: red_endgame as f64,
                    penalties: red_penalty as f64,
                },
                Alliance {
                    teams: blue_teams,
                    auto: match_info.score_blue_auto as f64,
                    teleop: (match_info.score_blue_final
                        - match_info.score_blue_auto
                        - match_info.score_red_foul
                        - blue_endgame as i64) as f64,
                    endgame: blue_endgame as f64,
                    penalties: blue_penalty as f64,
                },
            ));
        }

        if event.is_empty() {
            return Err(anyhow!("no scored qualification matches"));
        }

        let matches = matches_raw
            .iter()
            .flat_map(|m| MatchRow::rows_for(event_code, m))
            .collect();

        Ok((opr::solve(&event)?, matches))
    }

    async fn fetch_event_matches(
        client: &ApiClient,
        event_code: &str,
        year: i32,
    ) -> Result<Vec<MatchInfo>> {
        let params = ApiParams::new(vec![
            year.to_string(),
            "matches".to_owned(),
            event_code.to_owned(),
        ]);
        let resp: MatchesResponse = client.get(&params).await?;
        Ok(resp.matches)
    }

    async fn fetch_event_scores(
        client: &ApiClient,
        event_code: &str,
        year: i32,
    ) -> Result<Vec<MatchScore>> {
        let params = ApiParams::new(vec![
            year.to_string(),
            "scores".to_owned(),
            event_code.to_owned(),
            "qual".to_owned(),
        ]);
        let resp: ScoresResponse = client.get(&params).await?;
        Ok(resp.match_scores)
    }

    async fn fetch_team_info(client: &ApiClient, team_number: u32, year: i32) -> Result<Team> {
        let params = ApiParams::new(vec![year.to_string(), "teams".to_owned()])
            .with_query("teamNumber", team_number.to_string());
        let resp: TeamsResponse = client.get(&params).await?;
        resp.teams
            .first()
            .and_then(team_from_info)
            .ok_or_else(|| anyhow!("team {team_number} not found for season {year}"))
    }
}

fn started_on_or_after(date_start: Option<&str>, cutoff: chrono::NaiveDate) -> bool {
    date_start
        .and_then(|d| chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
        .is_some_and(|date| date >= cutoff)
}

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

fn team_from_info(team: &TeamInfo) -> Option<Team> {
    let unknown = |value: &Option<String>| value.clone().unwrap_or_else(|| "Unknown".to_owned());

    Some(Team {
        number: team.team_number?,
        name: unknown(&team.name_short),
        sponsors: unknown(&team.name_full)
            .replace("/", ", ")
            .replace("&", ", "),
        location: format!(
            "{}, {}, {}",
            unknown(&team.city),
            unknown(&team.state_prov),
            unknown(&team.country)
        ),
        founded: team.rookie_year,
        website: team.website.clone(),
        ..Default::default()
    })
}

#[cfg(test)]
mod event_filter_tests {
    use super::*;

    fn day(s: &str) -> chrono::NaiveDate {
        chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    #[test]
    fn an_event_on_the_cutoff_is_kept() {
        assert!(started_on_or_after(Some("2026-01-10"), day("2026-01-10")));
    }

    #[test]
    fn an_earlier_event_is_dropped() {
        assert!(!started_on_or_after(Some("2026-01-09"), day("2026-01-10")));
    }

    #[test]
    fn an_undated_or_unparseable_event_is_dropped() {
        assert!(!started_on_or_after(None, day("2026-01-10")));
        assert!(!started_on_or_after(Some("soon"), day("2026-01-10")));
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
    fn a_team_keeps_its_roster_metadata_through_the_merge() {
        let mut base = roster(&[1]);
        base.get_mut(&1).unwrap().location = "Houston, TX, USA".into();
        let merged = merge_events(&base, &[("TXHOU", opr(&[1], &[10.0]))]);
        assert_eq!(merged[&1].location, "Houston, TX, USA");
    }
}
