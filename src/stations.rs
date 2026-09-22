use crate::api_types::StationEntry;

/// `(red, blue)` in station order; an absent or off-field team stays `0`.
pub fn alliance_teams(entries: &[StationEntry]) -> ([u32; 2], [u32; 2]) {
    let (mut red, mut blue) = ([0u32; 2], [0u32; 2]);

    for entry in entries {
        if !entry.on_field {
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

    fn station(number: u32, station: &str, on_field: bool) -> StationEntry {
        StationEntry {
            team_number: Some(number),
            station: station.to_owned(),
            on_field,
        }
    }

    #[test]
    fn teams_are_placed_by_station_not_by_order() {
        let entries = [
            station(4, "Blue2", true),
            station(1, "Red1", true),
            station(3, "Blue1", true),
            station(2, "Red2", true),
        ];

        assert_eq!(alliance_teams(&entries), ([1, 2], [3, 4]));
    }

    #[test]
    fn an_off_field_team_leaves_its_slot_empty() {
        let entries = [
            station(1, "Red1", true),
            station(2, "Red2", false),
            station(3, "Blue1", true),
            station(4, "Blue2", true),
        ];

        assert_eq!(alliance_teams(&entries), ([1, 0], [3, 4]));
    }

    #[test]
    fn a_lone_second_station_keeps_its_slot() {
        assert_eq!(
            alliance_teams(&[station(7, "Red2", true)]),
            ([0, 7], [0, 0])
        );
    }

    #[test]
    fn station_case_is_ignored() {
        let entries = [station(1, "RED1", true), station(2, "blue2", true)];
        assert_eq!(alliance_teams(&entries), ([1, 0], [0, 2]));
    }

    #[test]
    fn a_missing_team_list_yields_empty_alliances() {
        assert_eq!(alliance_teams(&[]), ([0, 0], [0, 0]));
    }

    #[test]
    fn unknown_stations_are_ignored() {
        let entries = [
            station(1, "Red1", true),
            station(9, "Green1", true),
            station(7, "", true),
        ];

        assert_eq!(alliance_teams(&entries), ([1, 0], [0, 0]));
    }
}
