use anyhow::Result;
use model::prelude::{Supabase, Team};

use crate::matches::MatchRow;
use chrono::Utc;
use model::tables;
use std::collections::HashMap;

use crate::config::Config;

const TIE_TOLERANCE: f64 = 1e-6;

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

    pub async fn merge_with_database(
        &self,
        mut teams: HashMap<u32, Team>,
        force_update: bool,
    ) -> Result<HashMap<u32, Team>> {
        let stored = self.client.teams(&self.table).await?;

        for row in stored {
            let Some(team) = teams.get_mut(&row.number) else {
                continue;
            };

            team.add_events(row.events_attended.clone());
            team.founded = team.founded.or(row.founded);
            team.website = team.website.take().or_else(|| row.website.clone());

            if !force_update && team.overall <= row.overall {
                *team = Team {
                    events_attended: std::mem::take(&mut team.events_attended),
                    founded: team.founded,
                    website: team.website.take(),
                    ..row
                };
            }
        }

        Ok(teams)
    }

    pub fn rank(teams: &mut HashMap<u32, Team>) {
        let mut team_list: Vec<&mut Team> = teams.values_mut().collect();

        Self::assign_rank(
            &mut team_list,
            |t| t.overall,
            |t, r| t.overall_rank = Some(r),
            true,
        );
        Self::assign_rank(
            &mut team_list,
            |t| t.auto,
            |t, r| t.auto_rank = Some(r),
            true,
        );
        Self::assign_rank(
            &mut team_list,
            |t| t.teleop,
            |t, r| t.tele_rank = Some(r),
            true,
        );
        Self::assign_rank(
            &mut team_list,
            |t| t.endgame,
            |t, r| t.endgame_rank = Some(r),
            true,
        );
        Self::assign_rank(
            &mut team_list,
            |t| t.penalties,
            |t, r| t.penalty_rank = Some(r),
            false,
        );
    }

    /// Standard competition ranking: 1, 1, 3 — never 1, 1, 2.
    fn assign_rank<F, S>(teams: &mut [&mut Team], score_fn: F, set_rank: S, reverse: bool)
    where
        F: Fn(&Team) -> f64,
        S: Fn(&mut Team, u32),
    {
        let mut ordered: Vec<(usize, f64)> = teams
            .iter()
            .map(|team| score_fn(team))
            .enumerate()
            .collect();
        ordered.sort_by(|(_, a), (_, b)| {
            if reverse {
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
            set_rank(teams[index], rank);
            previous = Some(score);
        }
    }

    pub async fn upsert_to_database(&self, teams: &mut HashMap<u32, Team>) -> Result<()> {
        let timestamp = Utc::now().format("%Y-%m-%dT%H:%M:%S%.3f").to_string();
        for team in teams.values_mut() {
            team.profile_update = Some(timestamp.clone());
        }

        let rows: Vec<&Team> = teams.values().collect();
        self.client.upsert(&self.table, &rows, None).await?;
        if !rows.is_empty() {
            tracing::info!("✅ Upserted {} teams to database", rows.len());
        }

        Ok(())
    }
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
}
