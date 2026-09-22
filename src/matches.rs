use crate::api_types::MatchInfo;
use crate::level::Level;
use crate::stations::alliance_teams;
use md5::{Digest, Md5};
use serde::Serialize;

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
    /// `matchcode` = `md5("{event}-{level}-{number}-{series}-{color}")`.
    fn hash(event: &str, level: Level, number: i64, series: i64, color: &str) -> String {
        let mut h = Md5::new();
        h.update(format!("{event}-{level}-{number}-{series}-{color}").as_bytes());
        h.finalize().iter().map(|b| format!("{b:02x}")).collect()
    }

    /// Byes dropped; every level kept.
    pub fn rows_for(event: &str, m: &MatchInfo) -> Vec<MatchRow> {
        // Unplayed matches have no start time.
        let Some(date) = m.actual_start_time.clone() else {
            return vec![];
        };

        let ([r1, r2], [b1, b2]) = alliance_teams(&m.teams);

        let red = MatchRow {
            matchcode: Self::hash(event, m.tournament_level, m.match_number, m.series, "red"),
            alliance: "red".to_owned(),
            team_1: r1 as i64,
            team_2: r2 as i64,
            total_points: m.score_red_final,
            tele: m.score_red_final - m.score_red_auto - m.score_blue_foul,
            penalty: m.score_red_foul,
            win: m.score_red_final > m.score_blue_final,
            date: date.clone(),
            match_type: m.tournament_level,
        };
        let blue = MatchRow {
            matchcode: Self::hash(event, m.tournament_level, m.match_number, m.series, "blue"),
            alliance: "blue".to_owned(),
            team_1: b1 as i64,
            team_2: b2 as i64,
            total_points: m.score_blue_final,
            tele: m.score_blue_final - m.score_blue_auto - m.score_red_foul,
            penalty: m.score_blue_foul,
            win: m.score_blue_final > m.score_red_final,
            date,
            match_type: m.tournament_level,
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

    fn parse(v: serde_json::Value) -> MatchInfo {
        serde_json::from_value(v).unwrap()
    }

    fn match_json() -> serde_json::Value {
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
        let rows = MatchRow::rows_for("E", &parse(match_json()));
        assert_ne!(rows[0].matchcode, rows[1].matchcode);
    }

    #[test]
    fn distinct_matches_get_distinct_codes() {
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
        let m1 = MatchRow::rows_for("E", &parse(base(1)));
        let m2 = MatchRow::rows_for("E", &parse(base(2)));
        assert_ne!(m1[0].matchcode, m2[0].matchcode);
    }

    #[test]
    fn builds_both_alliances_from_one_match() {
        let [red, blue]: [MatchRow; 2] = MatchRow::rows_for("USTXCMP", &parse(match_json()))
            .try_into()
            .unwrap();

        assert_eq!((red.team_1, red.team_2), (23854, 26260));
        assert_eq!((blue.team_1, blue.team_2), (11505, 30070));
        assert_eq!(red.total_points, 145);
        assert_eq!(blue.total_points, 137);
        assert_eq!(red.tele, 145 - 20 - 50);
        assert_eq!(blue.tele, 137 - 30);
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
        let rows = MatchRow::rows_for("E", &parse(m));
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].team_1, rows[0].team_2), (1, 0));
        assert_eq!(rows[0].alliance, "red");
    }

    #[test]
    fn a_bye_alliance_produces_no_row() {
        let m = json!({
            "actualStartTime": "2025-11-16T13:39:29.611+00:00",
            "scoreRedFinal": 10, "scoreBlueFinal": 0,
            "teams": [
                { "teamNumber": 1, "station": "Red1" },
                { "teamNumber": 2, "station": "Red2" },
            ]
        });
        let rows = MatchRow::rows_for("E", &parse(m));
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
        assert!(MatchRow::rows_for("E", &parse(m)).is_empty());
    }

    #[test]
    fn a_lone_second_station_keeps_its_slot() {
        let m = json!({
            "actualStartTime": "2025-11-16T13:39:29.611+00:00",
            "scoreRedFinal": 10, "scoreBlueFinal": 5,
            "teams": [{ "teamNumber": 7, "station": "Red2" }]
        });
        let rows = MatchRow::rows_for("E", &parse(m));
        assert_eq!(rows.len(), 1);
        assert_eq!((rows[0].team_1, rows[0].team_2), (0, 7));
    }

    #[test]
    fn an_off_field_team_is_not_recorded() {
        let m = json!({
            "actualStartTime": "2025-11-16T13:39:29.611+00:00",
            "scoreRedFinal": 10, "scoreBlueFinal": 5,
            "teams": [
                { "teamNumber": 1, "station": "Red1" },
                { "teamNumber": 2, "station": "Red2", "onField": false },
            ]
        });
        let rows = MatchRow::rows_for("E", &parse(m));
        assert_eq!((rows[0].team_1, rows[0].team_2), (1, 0));
    }

    #[test]
    fn a_match_without_a_team_list_is_skipped() {
        assert!(MatchRow::rows_for("E", &parse(json!({ "scoreRedFinal": 10 }))).is_empty());
    }

    #[test]
    fn rescoring_keeps_the_matchcode() {
        let mut m = match_json();
        let before = MatchRow::rows_for("E", &parse(m.clone()))[0]
            .matchcode
            .clone();
        m["scoreRedFinal"] = json!(146);
        let after = MatchRow::rows_for("E", &parse(m))[0].matchcode.clone();
        assert_eq!(before, after);
    }
}
