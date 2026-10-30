use anyhow::Result;
use chrono::Utc;
use model::prelude::{Supabase, Team};
use model::tables;
use std::collections::HashMap;

use crate::config::Config;
use crate::matches::MatchRow;

const TIE_TOLERANCE: f64 = 1e-6;

#[derive(Debug, Clone, Copy)]
enum Metric {
    Overall,
    Auto,
    Teleop,
    Endgame,
    Penalties,
}

impl Metric {
    const ALL: [Metric; 5] = [
        Metric::Overall,
        Metric::Auto,
        Metric::Teleop,
        Metric::Endgame,
        Metric::Penalties,
    ];

    fn score(self, team: &Team) -> f64 {
        match self {
            Metric::Overall => team.overall,
            Metric::Auto => team.auto,
            Metric::Teleop => team.teleop,
            Metric::Endgame => team.endgame,
            Metric::Penalties => team.penalties,
        }
    }

    fn rank(self, team: &mut Team) -> &mut Option<u32> {
        match self {
            Metric::Overall => &mut team.overall_rank,
            Metric::Auto => &mut team.auto_rank,
            Metric::Teleop => &mut team.tele_rank,
            Metric::Endgame => &mut team.endgame_rank,
            Metric::Penalties => &mut team.penalty_rank,
        }
    }

    fn highest_first(self) -> bool {
        match self {
            Metric::Overall | Metric::Auto | Metric::Teleop | Metric::Endgame => true,
            Metric::Penalties => false,
        }
    }
}

pub struct Processor {
    client: Supabase,
    table: String,
    match_table: String,
}

impl Processor {
    pub fn new(config: Config, year: i32) -> Self {
        Self {
            client: Supabase::new(&config.supabase_url, config.supabase_key),
            table: tables::season(year),
            match_table: tables::matches(year),
        }
    }

    pub async fn upsert_matches(&self, matches: &[MatchRow]) -> Result<()> {
        let count = matches.len();
        self.client
            .upsert(&self.match_table, matches, Some("matchcode"))
            .await?;
        if count > 0 {
            tracing::info!("✅ Upserted {count} match rows to database");
        }
        Ok(())
    }

    /// Every team already stored for the season.
    pub async fn stored(&self) -> Result<HashMap<u32, Team>> {
        let rows: Vec<Team> = self
            .client
            .select_all(&self.table, &[], "teamNumber")
            .await?;
        Ok(rows.into_iter().map(|t| (t.number, t)).collect())
    }

    /// The whole season: stored rows with this run's teams laid over them.
    pub fn merge(
        stored: &HashMap<u32, Team>,
        fetched: HashMap<u32, Team>,
        force_update: bool,
    ) -> HashMap<u32, Team> {
        let mut season = stored.clone();
        for (number, mut team) in fetched {
            if let Some(row) = stored.get(&number) {
                team.add_events(row.events_attended.clone());
                team.founded = team.founded.or(row.founded);
                team.website = team.website.take().or_else(|| row.website.clone());

                if !force_update && team.overall <= row.overall {
                    team = Team {
                        events_attended: std::mem::take(&mut team.events_attended),
                        founded: team.founded,
                        website: team.website.take(),
                        ..row.clone()
                    };
                }
            }
            season.insert(number, team);
        }
        season
    }

    pub fn rank(teams: &mut HashMap<u32, Team>) {
        let mut team_list: Vec<&mut Team> = teams.values_mut().collect();
        for metric in Metric::ALL {
            Self::assign_rank(&mut team_list, metric);
        }
    }

    /// Standard competition ranking: 1, 1, 3 — never 1, 1, 2.
    fn assign_rank(teams: &mut [&mut Team], metric: Metric) {
        let mut ordered: Vec<(usize, f64)> = teams
            .iter()
            .map(|team| metric.score(team))
            .enumerate()
            .collect();
        ordered.sort_by(|(_, a), (_, b)| {
            if metric.highest_first() {
                b.total_cmp(a)
            } else {
                a.total_cmp(b)
            }
        });

        let mut rank = 1;
        let mut previous = None;
        for (position, (index, score)) in ordered.into_iter().enumerate() {
            if previous.is_some_and(|p: f64| (score - p).abs() > TIE_TOLERANCE) {
                rank = position as u32 + 1;
            }
            *metric.rank(teams[index]) = Some(rank);
            previous = Some(score);
        }
    }

    /// Writes only the teams that differ from what is stored.
    pub async fn upsert_changed(
        &self,
        stored: &HashMap<u32, Team>,
        season: HashMap<u32, Team>,
    ) -> Result<()> {
        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3f").to_string();
        let rows: Vec<Team> = changed(stored, season)
            .map(|mut team| {
                team.profile_update = Some(timestamp.clone());
                team
            })
            .collect();

        self.client.upsert(&self.table, &rows, None).await?;
        tracing::info!("✅ Upserted {} changed teams to database", rows.len());
        Ok(())
    }
}

fn changed(
    stored: &HashMap<u32, Team>,
    season: HashMap<u32, Team>,
) -> impl Iterator<Item = Team> + '_ {
    season
        .into_values()
        .filter(move |team| stored.get(&team.number) != Some(team))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn team(number: u32, auto: f64, penalties: f64) -> Team {
        Team {
            auto,
            penalties,
            ..Team::new(number, format!("Team {number}"))
        }
    }

    fn ranked(teams: Vec<Team>) -> HashMap<u32, Team> {
        let mut map: HashMap<u32, Team> = teams.into_iter().map(|t| (t.number, t)).collect();
        Processor::rank(&mut map);
        map
    }

    #[test]
    fn higher_auto_ranks_first() {
        let map = ranked(vec![
            team(1, 10.0, 0.0),
            team(2, 30.0, 0.0),
            team(3, 20.0, 0.0),
        ]);

        assert_eq!(map[&2].auto_rank, Some(1));
        assert_eq!(map[&3].auto_rank, Some(2));
        assert_eq!(map[&1].auto_rank, Some(3));
    }

    #[test]
    fn fewer_penalties_ranks_first() {
        let map = ranked(vec![
            team(1, 0.0, 50.0),
            team(2, 0.0, 10.0),
            team(3, 0.0, 30.0),
        ]);

        assert_eq!(map[&2].penalty_rank, Some(1));
        assert_eq!(map[&3].penalty_rank, Some(2));
        assert_eq!(map[&1].penalty_rank, Some(3));
    }

    #[test]
    fn negative_scores_rank_below_positive() {
        let map = ranked(vec![
            team(1, -5.0, 0.0),
            team(2, 0.0, 0.0),
            team(3, 5.0, 0.0),
        ]);

        assert_eq!(map[&3].auto_rank, Some(1));
        assert_eq!(map[&2].auto_rank, Some(2));
        assert_eq!(map[&1].auto_rank, Some(3));
    }

    #[test]
    fn equal_scores_share_a_rank() {
        let map = ranked(vec![
            team(1, 10.0, 0.0),
            team(2, 10.0, 0.0),
            team(3, 5.0, 0.0),
        ]);

        assert_eq!(map[&1].auto_rank, Some(1));
        assert_eq!(map[&2].auto_rank, Some(1));
        assert_eq!(map[&3].auto_rank, Some(3));
    }

    #[test]
    fn fractional_differences_do_not_collapse_into_ties() {
        let map = ranked(vec![
            team(1, 6.3485, 0.0),
            team(2, 6.9999, 0.0),
            team(3, 6.1, 0.0),
        ]);

        assert_eq!(map[&2].auto_rank, Some(1));
        assert_eq!(map[&1].auto_rank, Some(2));
        assert_eq!(map[&3].auto_rank, Some(3));
    }

    #[test]
    fn overall_ranks_on_auto_plus_teleop() {
        let mut a = team(1, 10.0, 0.0);
        a.teleop = 5.0;
        a.recompute_overall();
        let mut b = team(2, 1.0, 0.0);
        b.teleop = 50.0;
        b.recompute_overall();

        let map = ranked(vec![a, b]);
        assert_eq!(map[&2].overall_rank, Some(1));
        assert_eq!(map[&1].overall_rank, Some(2));
    }

    #[test]
    fn a_tie_is_followed_by_the_next_ordinal_rank() {
        let map = ranked(vec![
            team(1, 10.0, 0.0),
            team(2, 10.0, 0.0),
            team(3, 10.0, 0.0),
            team(4, 1.0, 0.0),
        ]);

        assert_eq!(map[&1].auto_rank, Some(1));
        assert_eq!(map[&4].auto_rank, Some(4));
    }

    #[test]
    fn every_metric_gets_ranked() {
        let map = ranked(vec![team(1, 10.0, 1.0), team(2, 20.0, 2.0)]);
        let t = &map[&1];

        assert!(t.auto_rank.is_some());
        assert!(t.tele_rank.is_some());
        assert!(t.endgame_rank.is_some());
        assert!(t.penalty_rank.is_some());
        assert!(t.overall_rank.is_some());
    }

    fn stored(teams: Vec<Team>) -> HashMap<u32, Team> {
        teams.into_iter().map(|t| (t.number, t)).collect()
    }

    fn scored(number: u32, overall: f64, events: &[&str]) -> Team {
        let mut t = team(number, overall, 0.0);
        t.recompute_overall();
        t.add_events(events.iter().map(|e| e.to_string()));
        t
    }

    #[test]
    fn teams_absent_from_this_run_stay_in_the_season() {
        let before = stored(vec![scored(1, 10.0, &["A"]), scored(2, 20.0, &["B"])]);
        let season = Processor::merge(&before, stored(vec![scored(3, 5.0, &["C"])]), false);
        assert_eq!(season.len(), 3);
    }

    #[test]
    fn ranks_span_the_whole_season_not_just_this_run() {
        let before = stored(vec![scored(1, 50.0, &["A"]), scored(2, 40.0, &["B"])]);
        let mut season = Processor::merge(&before, stored(vec![scored(3, 45.0, &["C"])]), false);
        Processor::rank(&mut season);

        assert_eq!(season[&1].overall_rank, Some(1));
        assert_eq!(season[&3].overall_rank, Some(2));
        assert_eq!(season[&2].overall_rank, Some(3));
    }

    #[test]
    fn a_stronger_stored_event_survives_a_weaker_run() {
        let before = stored(vec![scored(1, 50.0, &["STRONG"])]);
        let season = Processor::merge(&before, stored(vec![scored(1, 5.0, &["WEAK"])]), false);
        assert_eq!(season[&1].auto, 50.0);
        assert_eq!(season[&1].events_attended, ["STRONG", "WEAK"]);
    }

    #[test]
    fn force_update_takes_this_run_over_the_stored_figures() {
        let before = stored(vec![scored(1, 50.0, &["STRONG"])]);
        let season = Processor::merge(&before, stored(vec![scored(1, 5.0, &["WEAK"])]), true);
        assert_eq!(season[&1].auto, 5.0);
    }

    #[test]
    fn only_changed_teams_are_written() {
        let before = stored(vec![scored(1, 50.0, &["A"]), scored(2, 40.0, &["B"])]);
        let mut season = before.clone();
        season.get_mut(&2).unwrap().overall_rank = Some(7);

        let written: Vec<u32> = changed(&before, season).map(|t| t.number).collect();
        assert_eq!(written, [2]);
    }
}
