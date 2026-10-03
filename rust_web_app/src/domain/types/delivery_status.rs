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

#[cfg(test)]
mod tests {
    use super::DeliveryStatus;

    #[test]
    fn as_str_and_is_ok() {
        assert_eq!(DeliveryStatus::Ok.as_str(), "ok");
        assert!(DeliveryStatus::Ok.is_ok());
        assert!(!DeliveryStatus::Failed.is_ok());
    }
}
