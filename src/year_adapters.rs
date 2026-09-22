use crate::api_types::{AllianceScore, MatchScore};

/// Per-season point buckets for endgame and penalties.
pub trait ScoreAdapter: Send + Sync {
    fn endgame_points(&self, score: &MatchScore) -> (i32, i32);
    fn penalties(&self, score: &MatchScore) -> (i32, i32);
}

pub struct DefaultModernAdapter;
pub struct LegacyPenaltyAdapter;
pub struct Decode2025Adapter;
pub struct Biobuzz2026Adapter;

fn red_and_blue(score: &MatchScore, pick: fn(&AllianceScore) -> i32) -> (i32, i32) {
    (pick(score.red()), pick(score.blue()))
}

impl ScoreAdapter for DefaultModernAdapter {
    fn endgame_points(&self, score: &MatchScore) -> (i32, i32) {
        red_and_blue(score, |a| a.teleop_park_points + a.teleop_ascent_points)
    }

    fn penalties(&self, score: &MatchScore) -> (i32, i32) {
        red_and_blue(score, |a| a.foul_points_committed)
    }
}

impl ScoreAdapter for LegacyPenaltyAdapter {
    fn endgame_points(&self, score: &MatchScore) -> (i32, i32) {
        red_and_blue(score, |a| a.endgame_points)
    }

    fn penalties(&self, score: &MatchScore) -> (i32, i32) {
        red_and_blue(score, |a| a.penalty_points)
    }
}

impl ScoreAdapter for Decode2025Adapter {
    fn endgame_points(&self, score: &MatchScore) -> (i32, i32) {
        red_and_blue(score, |a| a.teleop_base_points)
    }

    fn penalties(&self, score: &MatchScore) -> (i32, i32) {
        red_and_blue(score, |a| a.foul_points_committed)
    }
}

// TODO(BIOBUZZ): endgame key is a guess; confirm against the 2026 score schema.
impl ScoreAdapter for Biobuzz2026Adapter {
    fn endgame_points(&self, score: &MatchScore) -> (i32, i32) {
        red_and_blue(score, |a| a.teleop_base_points)
    }

    fn penalties(&self, score: &MatchScore) -> (i32, i32) {
        red_and_blue(score, |a| a.foul_points_committed)
    }
}

pub fn adapter_for_year(year: i32) -> &'static dyn ScoreAdapter {
    match year {
        2026 => &Biobuzz2026Adapter,
        2025 => &Decode2025Adapter,
        2019..=2023 => &LegacyPenaltyAdapter,
        _ => &DefaultModernAdapter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn score() -> MatchScore {
        serde_json::from_value(json!({
            "matchNumber": 1,
            "alliances": [
                {
                    "alliance": "Blue",
                    "teleopParkPoints": 3, "teleopAscentPoints": 4, "teleopBasePoints": 5,
                    "endgamePoints": 6, "foulPointsCommitted": 7, "penaltyPoints": 8
                },
                {
                    "alliance": "Red",
                    "teleopParkPoints": 10, "teleopAscentPoints": 20, "teleopBasePoints": 30,
                    "endgamePoints": 40, "foulPointsCommitted": 50, "penaltyPoints": 60
                },
            ]
        }))
        .unwrap()
    }

    #[test]
    fn the_modern_adapter_adds_park_to_ascent() {
        assert_eq!(DefaultModernAdapter.endgame_points(&score()), (30, 7));
        assert_eq!(DefaultModernAdapter.penalties(&score()), (50, 7));
    }

    #[test]
    fn the_legacy_adapter_reads_the_old_buckets() {
        assert_eq!(LegacyPenaltyAdapter.endgame_points(&score()), (40, 6));
        assert_eq!(LegacyPenaltyAdapter.penalties(&score()), (60, 8));
    }

    #[test]
    fn decode_reads_the_base_bucket() {
        assert_eq!(Decode2025Adapter.endgame_points(&score()), (30, 5));
        assert_eq!(Decode2025Adapter.penalties(&score()), (50, 7));
    }

    #[test]
    fn each_season_selects_its_own_adapter() {
        let empty = MatchScore::default();
        assert_eq!(adapter_for_year(2021).penalties(&empty), (0, 0));
        assert_eq!(adapter_for_year(2025).endgame_points(&score()), (30, 5));
        assert_eq!(adapter_for_year(2022).endgame_points(&score()), (40, 6));
        assert_eq!(adapter_for_year(2024).endgame_points(&score()), (30, 7));
    }
}
