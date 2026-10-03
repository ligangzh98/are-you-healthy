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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertChannel {
    Feishu,
    Pushplus,
}

impl AlertChannel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Feishu => "feishu",
            Self::Pushplus => "pushplus",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryStatus {
    Ok,
    Failed,
}

impl DeliveryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Failed => "failed",
        }
    }

    pub fn is_ok(self) -> bool {
        matches!(self, Self::Ok)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointKind {
    Contains,
    Equals,
    NotContains,
    Regex,
    NotRegex,
}

impl CheckpointKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::Equals => "equals",
            Self::NotContains => "not_contains",
            Self::Regex => "regex",
            Self::NotRegex => "not_regex",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "contains" => Some(Self::Contains),
            "equals" => Some(Self::Equals),
            "not_contains" => Some(Self::NotContains),
            "regex" => Some(Self::Regex),
            "not_regex" => Some(Self::NotRegex),
            _ => None,
        }
    }
}
