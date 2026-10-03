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

#[cfg(test)]
mod tests {
    use super::CheckpointKind;

    #[test]
    fn roundtrip() {
        for s in [
            "contains",
            "equals",
            "not_contains",
            "regex",
            "not_regex",
        ] {
            let parsed = CheckpointKind::parse(s).unwrap();
            assert_eq!(parsed.as_str(), s);
        }
        assert_eq!(CheckpointKind::parse("REGEX"), Some(CheckpointKind::Regex));
    }
}
