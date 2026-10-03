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
