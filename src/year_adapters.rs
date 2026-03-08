use serde_json::Value;

pub trait ScoreAdapter: Send + Sync {
    fn endgame_points(&self, score: &Value) -> (i32, i32);
    fn penalties(&self, score: &Value) -> (i32, i32);
}

pub struct DefaultModernAdapter;
pub struct LegacyPenaltyAdapter;
pub struct Decode2025Adapter;

impl ScoreAdapter for DefaultModernAdapter {
    fn endgame_points(&self, score: &Value) -> (i32, i32) {
        let red = score["alliances"][1]["teleopParkPoints"].as_i64().unwrap_or(0)
            + score["alliances"][1]["teleopAscentPoints"].as_i64().unwrap_or(0);
        let blue = score["alliances"][0]["teleopParkPoints"].as_i64().unwrap_or(0)
            + score["alliances"][0]["teleopAscentPoints"].as_i64().unwrap_or(0);
        (red as i32, blue as i32)
    }

    fn penalties(&self, score: &Value) -> (i32, i32) {
        let red = score["alliances"][1]["foulPointsCommitted"].as_i64().unwrap_or(0);
        let blue = score["alliances"][0]["foulPointsCommitted"].as_i64().unwrap_or(0);
        (red as i32, blue as i32)
    }
}

impl ScoreAdapter for LegacyPenaltyAdapter {
    fn endgame_points(&self, score: &Value) -> (i32, i32) {
        let red = score["alliances"][1]["endgamePoints"].as_i64().unwrap_or(0);
        let blue = score["alliances"][0]["endgamePoints"].as_i64().unwrap_or(0);
        (red as i32, blue as i32)
    }

    fn penalties(&self, score: &Value) -> (i32, i32) {
        let red = score["alliances"][1]["penaltyPoints"].as_i64().unwrap_or(0);
        let blue = score["alliances"][0]["penaltyPoints"].as_i64().unwrap_or(0);
        (red as i32, blue as i32)
    }
}

impl ScoreAdapter for Decode2025Adapter {
    fn endgame_points(&self, score: &Value) -> (i32, i32) {
        let red = score["alliances"][1]["teleopBasePoints"].as_i64().unwrap_or(0);
        let blue = score["alliances"][0]["teleopBasePoints"].as_i64().unwrap_or(0);
        (red as i32, blue as i32)
    }

    fn penalties(&self, score: &Value) -> (i32, i32) {
        let red = score["alliances"][1]["foulPointsCommitted"].as_i64().unwrap_or(0);
        let blue = score["alliances"][0]["foulPointsCommitted"].as_i64().unwrap_or(0);
        (red as i32, blue as i32)
    }
}

pub fn adapter_for_year(year: i32) -> Box<dyn ScoreAdapter> {
    match year {
        2025 => Box::new(Decode2025Adapter),
        2019..=2023 => Box::new(LegacyPenaltyAdapter),
        _ => Box::new(DefaultModernAdapter),
    }
}
