use crate::api_types::StationEntry;

/// `(red, blue)` in station order; an absent or disqualified team stays `0`.
pub fn alliance_teams(entries: &[StationEntry]) -> ([u32; 2], [u32; 2]) {
    let (mut red, mut blue) = ([0u32; 2], [0u32; 2]);

    for entry in entries {
        if entry.dq {
            continue;
        }
        let Some(number) = entry.team_number else {
            continue;
        };
        let station = entry.station.to_ascii_lowercase();
        let slot = match station.chars().last() {
            Some('1') => 0,
            Some('2') => 1,
            _ => continue,
        };

        if station.starts_with("red") {
            red[slot] = number;
        } else if station.starts_with("blue") {
            blue[slot] = number;
        }
    }

    (red, blue)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn station(number: u32, station: &str) -> StationEntry {
        StationEntry {
            team_number: Some(number),
            station: station.to_owned(),
            dq: false,
        }
    }

    fn disqualified(number: u32, station: &str) -> StationEntry {
        StationEntry {
            dq: true,
            ..self::station(number, station)
        }
    }

    #[test]
    fn teams_are_placed_by_station_not_by_order() {
        let entries = [
            station(4, "Blue2"),
            station(1, "Red1"),
            station(3, "Blue1"),
            station(2, "Red2"),
        ];

        assert_eq!(alliance_teams(&entries), ([1, 2], [3, 4]));
    }

    #[test]
    fn a_disqualified_team_leaves_its_slot_empty() {
        let entries = [
            station(1, "Red1"),
            disqualified(2, "Red2"),
            station(3, "Blue1"),
            station(4, "Blue2"),
        ];

        assert_eq!(alliance_teams(&entries), ([1, 0], [3, 4]));
    }

    #[test]
    fn a_lone_second_station_keeps_its_slot() {
        assert_eq!(alliance_teams(&[station(7, "Red2")]), ([0, 7], [0, 0]));
    }

    #[test]
    fn station_case_is_ignored() {
        let entries = [station(1, "RED1"), station(2, "blue2")];
        assert_eq!(alliance_teams(&entries), ([1, 0], [0, 2]));
    }

    #[test]
    fn a_missing_team_list_yields_empty_alliances() {
        assert_eq!(alliance_teams(&[]), ([0, 0], [0, 0]));
    }

    #[test]
    fn unknown_stations_are_ignored() {
        let entries = [station(1, "Red1"), station(9, "Green1"), station(7, "")];

        assert_eq!(alliance_teams(&entries), ([1, 0], [0, 0]));
    }
}
