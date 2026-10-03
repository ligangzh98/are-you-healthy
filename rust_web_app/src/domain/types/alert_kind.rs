use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertKind {
    Down,
    Recovery,
    AlivePing,
    Test,
}

impl AlertKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Down => "down",
            Self::Recovery => "recovery",
            Self::AlivePing => "alive_ping",
            Self::Test => "test",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim() {
            "down" => Some(Self::Down),
            "recovery" => Some(Self::Recovery),
            "alive_ping" => Some(Self::AlivePing),
            "test" => Some(Self::Test),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AlertKind;

    #[test]
    fn roundtrip() {
        for s in ["down", "recovery", "alive_ping", "test"] {
            let parsed = AlertKind::parse(s).unwrap();
            assert_eq!(parsed.as_str(), s);
        }
        assert!(AlertKind::parse("  down  ").is_some());
        assert!(AlertKind::parse("all").is_none());
    }
}
