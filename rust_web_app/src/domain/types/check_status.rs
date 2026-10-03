use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CheckStatus {
    Up,
    Down,
    Error,
}

impl CheckStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Up => "up",
            Self::Down => "down",
            Self::Error => "error",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "up" => Some(Self::Up),
            "down" => Some(Self::Down),
            "error" => Some(Self::Error),
            _ => None,
        }
    }

    pub fn is_down(self) -> bool {
        matches!(self, Self::Down)
    }
}

#[cfg(test)]
mod tests {
    use super::CheckStatus;

    #[test]
    fn roundtrip() {
        for s in ["up", "down", "error"] {
            let parsed = CheckStatus::parse(s).unwrap();
            assert_eq!(parsed.as_str(), s);
        }
        assert!(CheckStatus::parse("UP").is_none());
    }
}
