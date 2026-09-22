use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Level {
    Practice,
    Qualification,
    Playoff,
    #[default]
    Unknown,
}

impl Level {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Practice => "PRACTICE",
            Self::Qualification => "QUALIFICATION",
            Self::Playoff => "PLAYOFF",
            Self::Unknown => "Unknown",
        }
    }

    /// Unrecognized input becomes [`Level::Unknown`].
    pub fn from_api(level: &str) -> Self {
        match level {
            "PRACTICE" => Self::Practice,
            "QUALIFICATION" => Self::Qualification,
            "PLAYOFF" => Self::Playoff,
            _ => Self::Unknown,
        }
    }
}

impl std::fmt::Display for Level {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Serialize for Level {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Level {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        Ok(Self::from_api(&String::deserialize(d)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_the_api_string() {
        for s in ["PRACTICE", "QUALIFICATION", "PLAYOFF"] {
            assert_eq!(Level::from_api(s).as_str(), s);
        }
    }

    #[test]
    fn unknown_strings_fall_through() {
        assert_eq!(Level::from_api("SOMETHING_NEW"), Level::Unknown);
        assert_eq!(Level::from_api(""), Level::Unknown);
    }

    #[test]
    fn deserializes_from_the_api_string() {
        let level: Level = serde_json::from_value(serde_json::json!("PLAYOFF")).unwrap();
        assert_eq!(level, Level::Playoff);
    }

    #[test]
    fn deserializing_an_unknown_string_does_not_fail() {
        let level: Level = serde_json::from_value(serde_json::json!("SOMETHING_NEW")).unwrap();
        assert_eq!(level, Level::Unknown);
    }

    #[test]
    fn serializes_as_its_string() {
        assert_eq!(
            serde_json::to_value(Level::Qualification).unwrap(),
            serde_json::json!("QUALIFICATION")
        );
    }
}
