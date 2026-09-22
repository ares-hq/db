use serde::Deserialize;

use crate::level::Level;

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TeamsResponse {
    pub page_total: Option<i64>,
    pub teams: Vec<TeamInfo>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct TeamInfo {
    pub team_number: Option<u32>,
    pub name_short: Option<String>,
    pub name_full: Option<String>,
    pub city: Option<String>,
    pub state_prov: Option<String>,
    pub country: Option<String>,
    pub rookie_year: Option<u16>,
    pub website: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EventsResponse {
    pub events: Vec<EventSummary>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EventSummary {
    pub code: Option<String>,
    pub date_start: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchesResponse {
    pub matches: Vec<MatchInfo>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchInfo {
    pub tournament_level: Level,
    pub match_number: i64,
    pub series: i64,
    /// Absent until the match is played.
    pub actual_start_time: Option<String>,
    pub score_red_final: i64,
    pub score_red_auto: i64,
    pub score_red_foul: i64,
    pub score_blue_final: i64,
    pub score_blue_auto: i64,
    pub score_blue_foul: i64,
    pub teams: Vec<StationEntry>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct StationEntry {
    pub team_number: Option<u32>,
    pub station: String,
    /// Absent means the team played; only a surrogate is flagged `false`.
    #[serde(default = "yes")]
    pub on_field: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ScoresResponse {
    pub match_scores: Vec<MatchScore>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MatchScore {
    pub match_number: Option<i64>,
    pub alliances: Vec<AllianceScore>,
}

static EMPTY_ALLIANCE: AllianceScore = AllianceScore {
    alliance: String::new(),
    teleop_park_points: 0,
    teleop_ascent_points: 0,
    teleop_base_points: 0,
    endgame_points: 0,
    foul_points_committed: 0,
    penalty_points: 0,
};

impl MatchScore {
    pub fn red(&self) -> &AllianceScore {
        self.side("red", 1)
    }

    pub fn blue(&self) -> &AllianceScore {
        self.side("blue", 0)
    }

    /// Falls back to the historical position (`1` red, `0` blue) when unlabelled.
    fn side(&self, name: &str, position: usize) -> &AllianceScore {
        self.alliances
            .iter()
            .find(|a| a.alliance.eq_ignore_ascii_case(name))
            .or_else(|| self.alliances.get(position))
            .unwrap_or(&EMPTY_ALLIANCE)
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct AllianceScore {
    pub alliance: String,
    pub teleop_park_points: i32,
    pub teleop_ascent_points: i32,
    pub teleop_base_points: i32,
    pub endgame_points: i32,
    pub foul_points_committed: i32,
    #[serde(alias = "penaltyPointsCommitted")]
    pub penalty_points: i32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_match_missing_every_score_reads_as_zero_not_an_error() {
        let m: MatchInfo = serde_json::from_value(json!({ "matchNumber": 4 })).unwrap();
        assert_eq!(m.match_number, 4);
        assert_eq!(m.score_red_final, 0);
        assert_eq!(m.tournament_level, Level::Unknown);
        assert!(m.actual_start_time.is_none());
    }

    #[test]
    fn a_station_without_on_field_is_treated_as_playing() {
        let e: StationEntry =
            serde_json::from_value(json!({ "teamNumber": 1, "station": "Red1" })).unwrap();
        assert!(e.on_field);
    }

    #[test]
    fn alliances_are_matched_by_label_not_position() {
        let s: MatchScore = serde_json::from_value(json!({
            "matchNumber": 1,
            "alliances": [
                { "alliance": "Red", "endgamePoints": 30 },
                { "alliance": "Blue", "endgamePoints": 10 },
            ]
        }))
        .unwrap();

        assert_eq!(s.red().endgame_points, 30);
        assert_eq!(s.blue().endgame_points, 10);
    }

    #[test]
    fn unlabelled_alliances_fall_back_to_the_historical_positions() {
        let s: MatchScore = serde_json::from_value(json!({
            "alliances": [{ "endgamePoints": 10 }, { "endgamePoints": 30 }]
        }))
        .unwrap();

        assert_eq!(s.red().endgame_points, 30);
        assert_eq!(s.blue().endgame_points, 10);
    }

    #[test]
    fn a_missing_alliance_scores_nothing() {
        let s = MatchScore::default();
        assert_eq!(s.red().foul_points_committed, 0);
        assert_eq!(s.blue().teleop_base_points, 0);
    }

    #[test]
    fn a_team_row_keeps_what_it_has_and_nulls_the_rest() {
        let t: TeamInfo =
            serde_json::from_value(json!({ "teamNumber": 254, "nameShort": "Cheesy" })).unwrap();
        assert_eq!(t.team_number, Some(254));
        assert_eq!(t.name_short.as_deref(), Some("Cheesy"));
        assert!(t.city.is_none());
        assert!(t.rookie_year.is_none());
    }
}
