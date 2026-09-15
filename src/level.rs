use serde::{Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Practice,
    Qualification,
    Playoff,
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
    fn serializes_as_its_string() {
        assert_eq!(
            serde_json::to_value(Level::Qualification).unwrap(),
            serde_json::json!("QUALIFICATION")
        );
    }
}
