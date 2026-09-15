use serde_json::Value;

pub trait ScoreAdapter: Send + Sync {
    fn endgame_points(&self, score: &Value) -> (i32, i32);
    fn penalties(&self, score: &Value) -> (i32, i32);
}

pub struct DefaultModernAdapter;
pub struct LegacyPenaltyAdapter;
pub struct Decode2025Adapter;
pub struct Biobuzz2026Adapter;

/// Alliance `1` is red, `0` is blue.
fn field(score: &Value, alliance: usize, key: &str) -> i32 {
    score["alliances"][alliance][key].as_i64().unwrap_or(0) as i32
}

fn both(score: &Value, key: &str) -> (i32, i32) {
    (field(score, 1, key), field(score, 0, key))
}

impl ScoreAdapter for DefaultModernAdapter {
    fn endgame_points(&self, score: &Value) -> (i32, i32) {
        let sum = |a| field(score, a, "teleopParkPoints") + field(score, a, "teleopAscentPoints");
        (sum(1), sum(0))
    }

    fn penalties(&self, score: &Value) -> (i32, i32) {
        both(score, "foulPointsCommitted")
    }
}

impl ScoreAdapter for LegacyPenaltyAdapter {
    fn endgame_points(&self, score: &Value) -> (i32, i32) {
        both(score, "endgamePoints")
    }

    fn penalties(&self, score: &Value) -> (i32, i32) {
        both(score, "penaltyPoints")
    }
}

impl ScoreAdapter for Decode2025Adapter {
    fn endgame_points(&self, score: &Value) -> (i32, i32) {
        both(score, "teleopBasePoints")
    }

    fn penalties(&self, score: &Value) -> (i32, i32) {
        both(score, "foulPointsCommitted")
    }
}

// TODO(BIOBUZZ): endgame key is a guess; confirm against the 2026 score schema.
impl ScoreAdapter for Biobuzz2026Adapter {
    fn endgame_points(&self, score: &Value) -> (i32, i32) {
        both(score, "teleopBasePoints")
    }

    fn penalties(&self, score: &Value) -> (i32, i32) {
        both(score, "foulPointsCommitted")
    }
}

pub fn adapter_for_year(year: i32) -> Box<dyn ScoreAdapter> {
    match year {
        2026 => Box::new(Biobuzz2026Adapter),
        2025 => Box::new(Decode2025Adapter),
        2019..=2023 => Box::new(LegacyPenaltyAdapter),
        _ => Box::new(DefaultModernAdapter),
    }
}
