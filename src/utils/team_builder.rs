use std::collections::{HashMap, HashSet};

use ndarray::{Array1, Array2};

#[derive(Debug, Clone)]
pub struct MatchTeam {
    pub team_number: i32,
    pub station: String,
    pub on_field: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct MatchData {
    pub teams: Vec<MatchTeam>,
    pub description: String,
    pub actual_start_time: String,
    pub score_red_final: Option<i32>,
    pub score_red_auto: Option<i32>,
    pub score_blue_foul: Option<i32>,
    pub score_blue_final: Option<i32>,
    pub score_blue_auto: Option<i32>,
    pub score_red_foul: Option<i32>,
    pub score_red_endgame: Option<i32>,
    pub score_blue_endgame: Option<i32>,
    pub penalty_points_red: Option<i32>,
    pub penalty_points_blue: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct MatrixBuilder {
    pub matches: Vec<MatchData>,
    pub teams: Vec<i32>,
    pub num_matches: usize,
    pub num_teams: usize,
    pub binary_matrix: Array2<i32>,
    pub auto_matrix: Array1<i32>,
    pub tele_matrix: Array1<i32>,
    pub endgame_matrix: Array1<i32>,
    pub penalties_matrix: Array1<i32>,
    pub team_indices: HashMap<i32, usize>,
}

impl MatrixBuilder {
    /// Initializes matrices and immediately builds binary/score matrices,
    /// matching the Python constructor behavior.
    pub fn new(matches: Vec<MatchData>) -> Self {
        let num_matches = matches.len();

        let mut builder = Self {
            matches,
            teams: Vec::new(),
            num_matches,
            num_teams: 0,
            binary_matrix: Array2::zeros((num_matches * 2, 0)),
            auto_matrix: Array1::zeros(num_matches * 2),
            tele_matrix: Array1::zeros(num_matches * 2),
            endgame_matrix: Array1::zeros(num_matches * 2),
            penalties_matrix: Array1::zeros(num_matches * 2),
            team_indices: HashMap::new(),
        };

        builder.create_binary_and_score_matrices();
        builder
    }

    /// This function is necessary to find all teams in a given tournament.
    pub fn create_team_matrices(&mut self) {
        let mut seen = HashSet::with_capacity(self.num_matches.saturating_mul(6));
        self.teams.clear();

        for m in &self.matches {
            for team in &m.teams {
                if seen.insert(team.team_number) {
                    self.teams.push(team.team_number);
                }
            }
        }

        self.team_indices = self
            .teams
            .iter()
            .enumerate()
            .map(|(idx, team)| (*team, idx))
            .collect();

        self.num_teams = self.teams.len();
        self.binary_matrix = Array2::zeros((self.num_matches * 2, self.num_teams));
    }

    /// This function creates the binary and score matrices based on the
    /// team alliance scores in a given tournament.
    pub fn create_binary_and_score_matrices(&mut self) {
        // Make sure to initialize team matrices before creating binary/score matrices.
        self.create_team_matrices();

        for (match_idx, m) in self.matches.iter().enumerate() {
            let is_qualification = m.description.contains("Qualification");
            let is_legacy_event = m.actual_start_time.as_str() < "2021-08-01";

            let red_score = m.score_red_final.unwrap_or(0)
                - m.score_red_auto.unwrap_or(0)
                - m.score_blue_foul.unwrap_or(0);
            let blue_score = m.score_blue_final.unwrap_or(0)
                - m.score_blue_auto.unwrap_or(0)
                - m.score_red_foul.unwrap_or(0);
            let red_score_auto = m.score_red_auto.unwrap_or(0);
            let blue_score_auto = m.score_blue_auto.unwrap_or(0);
            let red_score_endgame = m.score_red_endgame.unwrap_or(0);
            let blue_score_endgame = m.score_blue_endgame.unwrap_or(0);
            let red_score_penalties = m.penalty_points_red.unwrap_or(0);
            let blue_score_penalties = m.penalty_points_blue.unwrap_or(0);

            for team in &m.teams {
                if let Some(team_idx) = self.team_indices.get(&team.team_number) {
                    // It checks if a team is on the field during this match.
                    // This is not used in normal OPR calculations and may change.
                    let include_team =
                        is_qualification && (team.on_field.unwrap_or(true) || is_legacy_event);

                    if include_team {
                        if team.station.starts_with("Red") {
                            self.binary_matrix[[2 * match_idx, *team_idx]] = 1;
                        } else if team.station.starts_with("Blue") {
                            self.binary_matrix[[2 * match_idx + 1, *team_idx]] = 1;
                        }
                    }
                }
            }

            self.tele_matrix[2 * match_idx] = red_score;
            self.tele_matrix[2 * match_idx + 1] = blue_score;
            self.auto_matrix[2 * match_idx] = red_score_auto;
            self.auto_matrix[2 * match_idx + 1] = blue_score_auto;
            self.endgame_matrix[2 * match_idx] = red_score_endgame;
            self.endgame_matrix[2 * match_idx + 1] = blue_score_endgame;
            self.penalties_matrix[2 * match_idx] = red_score_penalties;
            self.penalties_matrix[2 * match_idx + 1] = blue_score_penalties;
        }
    }
}
