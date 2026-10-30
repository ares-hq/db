use std::collections::HashMap;

use anyhow::Result;
use model::prelude::{Alliance, Event, Match};
use nalgebra::DMatrix;

use crate::utils::matrix_math::least_squares;

/// `(team, [auto, teleop, endgame, penalties])` in event team order.
#[derive(Debug, Clone, PartialEq)]
pub struct Opr(pub Vec<(u32, [f64; 4])>);

impl Opr {
    pub fn iter(&self) -> impl Iterator<Item = (u32, [f64; 4])> + '_ {
        self.0.iter().copied()
    }

    pub fn teams(&self) -> impl Iterator<Item = u32> + '_ {
        self.0.iter().map(|&(number, _)| number)
    }
}

pub fn solve(event: &Event) -> Result<Opr> {
    let teams = event.teams();
    let column: HashMap<u32, usize> = teams.iter().enumerate().map(|(i, &t)| (t, i)).collect();

    let alliances: Vec<&Alliance> = event.matches.iter().flat_map(Match::alliances).collect();

    let mut design = DMatrix::zeros(alliances.len(), teams.len());
    for (row, alliance) in alliances.iter().enumerate() {
        for number in alliance.present_teams() {
            design[(row, column[&number])] = 1.0;
        }
    }
    let scores = DMatrix::from_fn(alliances.len(), 4, |row, phase| {
        let a = alliances[row];
        [a.auto, a.teleop, a.endgame, a.penalties][phase]
    });

    let solved = least_squares(design, &scores)?;
    Ok(Opr(teams
        .into_iter()
        .enumerate()
        .map(|(i, number)| (number, std::array::from_fn(|phase| solved[(i, phase)])))
        .collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn flat(teams: [u32; 2], score: f64) -> Alliance {
        Alliance {
            teams,
            auto: score,
            teleop: score,
            endgame: score,
            penalties: score,
        }
    }

    fn all_pairs(n: u32, contribution: impl Fn(u32) -> f64) -> Event {
        let pairs: Vec<[u32; 2]> = (1..=n)
            .flat_map(|i| (i + 1..=n).map(move |j| [i, j]))
            .collect();

        let mut event = Event::new("TEST");
        for chunk in pairs.chunks(2) {
            let score = |p: [u32; 2]| contribution(p[0]) + contribution(p[1]);
            let red = chunk[0];
            let blue = *chunk.get(1).unwrap_or(&red);
            event.add_match(Match::new(flat(red, score(red)), flat(blue, score(blue))));
        }
        event
    }

    #[test]
    fn recovers_known_contributions() {
        let event = all_pairs(6, |t| t as f64 * 10.0);
        let opr = solve(&event).unwrap();

        for (number, [auto, teleop, endgame, penalties]) in opr.iter() {
            let want = number as f64 * 10.0;
            for (got, label) in [
                (auto, "auto"),
                (teleop, "teleop"),
                (endgame, "endgame"),
                (penalties, "penalties"),
            ] {
                assert!(
                    (got - want).abs() < 1e-9,
                    "team {number} {label}: {got} vs {want}"
                );
            }
        }
    }

    #[test]
    fn columns_follow_event_team_order() {
        let event = all_pairs(6, |t| t as f64);
        let opr = solve(&event).unwrap();
        assert_eq!(opr.teams().collect::<Vec<_>>(), event.teams());
    }

    #[test]
    fn opr_goes_negative_for_a_team_carried_by_partners() {
        let event = all_pairs(6, |t| if t == 1 { -25.0 } else { 50.0 });
        let opr = solve(&event).unwrap();

        let (_, [auto, ..]) = opr.iter().find(|(n, _)| *n == 1).unwrap();
        assert!((auto + 25.0).abs() < 1e-9, "expected -25, got {auto}");
    }

    #[test]
    fn an_empty_slot_is_not_a_team() {
        let mut event = all_pairs(4, |_| 25.0);
        event.add_match(Match::new(flat([1, 0], 25.0), flat([2, 3], 50.0)));

        let opr = solve(&event).unwrap();

        assert!(
            !opr.teams().any(|n| n == 0),
            "slot 0 became a column: {opr:?}"
        );
        for (number, [auto, ..]) in opr.iter() {
            assert!((auto - 25.0).abs() < 1e-9, "team {number}: {auto} vs 25");
        }
    }

    #[test]
    fn an_event_with_more_teams_than_alliances_is_rejected() {
        let mut event = Event::new("TEST");
        event.add_match(Match::new(flat([1, 2], 10.0), flat([3, 4], 10.0)));
        event.add_match(Match::new(flat([5, 6], 10.0), flat([7, 8], 10.0)));

        let err = solve(&event).unwrap_err().to_string();
        assert!(err.contains("underdetermined"), "{err}");
    }

    #[test]
    fn an_event_with_no_matches_is_rejected() {
        assert!(solve(&Event::new("TEST")).is_err());
    }
}
