//! Raw per-alliance match results for the `matches_<year>` tables — the finals
//! as reported, one row per alliance. Distinct from [`model::prelude::Match`]
//! (the OPR view, which re-weights scores). Keyed by `matchcode` on match
//! identity, not score, so a re-scored match updates in place.

use crate::level::Level;
use md5::{Digest, Md5};
use serde::Serialize;
use serde_json::Value;

/// One alliance's result in one match, as a `matches_<year>` row.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MatchRow {
    pub matchcode: String,
    pub alliance: String,
    pub team_1: i64,
    pub team_2: i64,
    #[serde(rename = "totalPoints")]
    pub total_points: i64,
    pub tele: i64,
    pub penalty: i64,
    pub win: bool,
    pub date: String,
    #[serde(rename = "matchType")]
    pub match_type: Level,
}

impl MatchRow {
    /// `matchcode` = `md5("{event}-{level}-{number}-{series}-{color}")` — identity, no score.
    fn hash(event: &str, level: Level, number: i64, series: i64, color: &str) -> String {
        let mut h = Md5::new();
        h.update(format!("{event}-{level}-{number}-{series}-{color}").as_bytes());
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    /// `color`'s two teams in station order, `0`-padded.
    fn pick_two(teams: &[Value], color: &str) -> (i64, i64) {
        let mut picked = teams
            .iter()
            .filter(|t| {
                t["station"]
                    .as_str()
                    .is_some_and(|s| s.to_ascii_lowercase().starts_with(color))
            })
            .filter_map(|t| t["teamNumber"].as_i64())
            .take(2);
        (picked.next().unwrap_or(0), picked.next().unwrap_or(0))
    }

    /// One row per alliance that fielded a team (byes dropped); every level kept.
    pub fn from_match(event: &str, m: &Value) -> Vec<MatchRow> {
        let Some(teams) = m["teams"].as_array() else {
            return vec![];
        };
        // Unplayed matches have no start time; skip so `date` (timestamptz) stays valid.
        let Some(date) = m["actualStartTime"].as_str().map(str::to_owned) else {
            return vec![];
        };
        let int = |key: &str| m[key].as_i64().unwrap_or(0);

        let (red_final, blue_final) = (int("scoreRedFinal"), int("scoreBlueFinal"));
        let match_type = Level::from_api(m["tournamentLevel"].as_str().unwrap_or(""));
        let number = int("matchNumber");
        let series = int("series");

        let (r1, r2) = Self::pick_two(teams, "red");
        let (b1, b2) = Self::pick_two(teams, "blue");

        let red = MatchRow {
            matchcode: Self::hash(event, match_type, number, series, "red"),
            alliance: "red".to_owned(),
            team_1: r1,
            team_2: r2,
            total_points: red_final,
            tele: red_final - int("scoreRedAuto") - int("scoreBlueFoul"),
            penalty: int("scoreRedFoul"),
            win: red_final > blue_final,
            date: date.clone(),
            match_type,
        };
        let blue = MatchRow {
            matchcode: Self::hash(event, match_type, number, series, "blue"),
            alliance: "blue".to_owned(),
            team_1: b1,
            team_2: b2,
            total_points: blue_final,
            tele: blue_final - int("scoreBlueAuto") - int("scoreRedFoul"),
            penalty: int("scoreBlueFoul"),
            win: blue_final > red_final,
            date,
            match_type,
        };
        [red, blue]
            .into_iter()
            .filter(|r| r.team_1 != 0 || r.team_2 != 0)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn match_json() -> Value {
        json!({
            "actualStartTime": "2025-11-16T13:39:29.611+00:00",
            "tournamentLevel": "QUALIFICATION",
            "matchNumber": 12, "series": 0,
            "scoreRedFinal": 145, "scoreRedAuto": 20, "scoreRedFoul": 0,
            "scoreBlueFinal": 137, "scoreBlueAuto": 30, "scoreBlueFoul": 50,
            "teams": [
                { "teamNumber": 23854, "station": "Red1" },
                { "teamNumber": 26260, "station": "Red2" },
                { "teamNumber": 11505, "station": "Blue1" },
                { "teamNumber": 30070, "station": "Blue2" },
            ]
        })
    }

    #[test]
    fn hash_is_the_match_identity_not_its_score() {
        assert_eq!(
            MatchRow::hash("USTXCMP", Level::Qualification, 5, 0, "red"),
            "1093f8f42e17f22f44613527718f893f"
        );
    }

    #[test]
    fn each_alliance_of_a_match_gets_its_own_code() {
        let rows = MatchRow::from_match("E", &match_json());
        assert_ne!(rows[0].matchcode, rows[1].matchcode);
    }

    #[test]
    fn distinct_matches_get_distinct_codes() {
        // Same alliance pairing, different playoff matches (Finals 1 vs 2)
        // must not collapse onto one row.
        let base = |n| {
            json!({
                "actualStartTime": "2025-11-16T13:39:29.611+00:00",
                "tournamentLevel": "PLAYOFF", "matchNumber": n, "series": 1,
                "scoreRedFinal": 100, "scoreBlueFinal": 90,
                "teams": [
                    { "teamNumber": 1, "station": "Red1" },
                    { "teamNumber": 2, "station": "Red2" },
                    { "teamNumber": 3, "station": "Blue1" },
                    { "teamNumber": 4, "station": "Blue2" },
                ]
            })
        };
        let m1 = MatchRow::from_match("E", &base(1));
        let m2 = MatchRow::from_match("E", &base(2));
        assert_ne!(m1[0].matchcode, m2[0].matchcode);
    }

    #[test]
    fn builds_both_alliances_from_one_match() {
        let [red, blue]: [MatchRow; 2] = MatchRow::from_match("USTXCMP", &match_json())
            .try_into()
            .unwrap();

        assert_eq!((red.team_1, red.team_2), (23854, 26260));
        assert_eq!((blue.team_1, blue.team_2), (11505, 30070));
        assert_eq!(red.total_points, 145);
        assert_eq!(blue.total_points, 137);
        // tele = final - auto - opponent foul.
        assert_eq!(red.tele, 145 - 20 - 50);
        assert_eq!(blue.tele, 137 - 30 - 0);
        assert_eq!(red.penalty, 0);
        assert_eq!(blue.penalty, 50);
        assert!(red.win);
        assert!(!blue.win);
    }

    #[test]
    fn a_short_alliance_keeps_its_lone_team_and_drops_the_empty_side() {
        let m = json!({
            "actualStartTime": "2025-11-16T13:39:29.611+00:00",
            "scoreRedFinal": 10, "scoreBlueFinal": 5,
            "teams": [{ "teamNumber": 1, "station": "Red1" }]
        });
        let rows = MatchRow::from_match("E", &m);
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].team_1, rows[0].team_2), (1, 0));
        assert_eq!(rows[0].alliance, "red");
    }

    #[test]
    fn a_bye_alliance_produces_no_row() {
        // Blue has no teams (a playoff bye): its all-zero row must not be written.
        let m = json!({
            "actualStartTime": "2025-11-16T13:39:29.611+00:00",
            "scoreRedFinal": 10, "scoreBlueFinal": 0,
            "teams": [
                { "teamNumber": 1, "station": "Red1" },
                { "teamNumber": 2, "station": "Red2" },
            ]
        });
        let rows = MatchRow::from_match("E", &m);
        assert_eq!(rows.len(), 1);
        assert!(rows.iter().all(|r| r.team_1 != 0 || r.team_2 != 0));
    }

    #[test]
    fn an_unplayed_match_without_a_start_time_is_skipped() {
        let m = json!({
            "tournamentLevel": "QUALIFICATION", "matchNumber": 1, "series": 0,
            "teams": [{ "teamNumber": 1, "station": "Red1" }]
            // no actualStartTime, no scores
        });
        assert!(MatchRow::from_match("E", &m).is_empty());
    }

    #[test]
    fn a_match_without_a_team_list_is_skipped() {
        assert!(MatchRow::from_match("E", &json!({ "scoreRedFinal": 10 })).is_empty());
    }

    #[test]
    fn rescoring_keeps_the_matchcode() {
        // A revised score must update the existing row, not insert a new one.
        let mut m = match_json();
        let before = MatchRow::from_match("E", &m)[0].matchcode.clone();
        m["scoreRedFinal"] = json!(146);
        let after = MatchRow::from_match("E", &m)[0].matchcode.clone();
        assert_eq!(before, after);
    }
}
