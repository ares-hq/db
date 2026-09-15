use anyhow::Result;
use model::prelude::{Supabase, Team};

use crate::matches::MatchRow;
use chrono::Utc;
use std::collections::HashMap;

use crate::config::Config;

pub struct Processor {
    client: Supabase,
    table: String,
    match_table: String,
}

impl Processor {
    pub fn new(config: Config, year: i32) -> Self {
        Self {
            client: Supabase::new(&config.supabase_url, config.supabase_key),
            table: format!("season_{year}"),
            match_table: format!("matches_{year}"),
        }
    }

    /// Upserts raw match results, keyed on `matchcode`.
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

            // The stored row came from a stronger event; keep it.
            if !force_update && team.overall <= row.overall {
                *team = Team {
                    events_attended: std::mem::take(&mut team.events_attended),
                    founded: team.founded,
                    website: team.website.take(),
                    last_match: team.last_match,
                    last_checked: team.last_checked,
                    ..row
                };
            }
        }

        Ok(teams)
    }

    pub fn rank(teams: &mut HashMap<u32, Team>) {
        let mut team_list: Vec<&mut Team> = teams.values_mut().collect();

        // Higher is better everywhere except penalties.
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

    fn assign_rank<F, S>(teams: &mut [&mut Team], score_fn: F, set_rank: S, reverse: bool)
    where
        F: Fn(&Team) -> f64,
        S: Fn(&mut Team, u32),
    {
        teams.sort_by(|a, b| {
            let (sa, sb) = (score_fn(a), score_fn(b));
            if reverse {
                sb.total_cmp(&sa)
            } else {
                sa.total_cmp(&sb)
            }
        });

        let mut current_rank = 1;
        for i in 0..teams.len() {
            if i > 0 && (score_fn(teams[i]) - score_fn(teams[i - 1])).abs() > 1e-6 {
                current_rank = (i + 1) as u32;
            }
            set_rank(teams[i], current_rank);
        }
    }

    pub async fn upsert_to_database(&self, teams: &HashMap<u32, Team>) -> Result<()> {
        let now = Utc::now();
        let timestamp = now.format("%Y-%m-%dT%H:%M:%S%.3f").to_string();

        let rows: Vec<Team> = teams
            .values()
            .map(|team| Team {
                profile_update: Some(timestamp.clone()),
                ..team.clone()
            })
            .collect();

        let count = rows.len();
        self.client.upsert(&self.table, &rows, None).await?;
        if count > 0 {
            tracing::info!("✅ Upserted {count} teams to database");
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
