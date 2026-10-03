use crate::types::CheckpointKind;
use regex::Regex;

#[derive(Debug, Clone)]
pub struct CheckpointRule {
    pub kind: CheckpointKind,
    pub value: String,
}

pub fn normalize_kind(kind: &str) -> Option<CheckpointKind> {
    CheckpointKind::parse(kind)
}

/// 对响应体执行全部检查点，返回第一条失败说明。
pub fn evaluate_all(body: &str, rules: &[CheckpointRule]) -> Option<String> {
    for (i, rule) in rules.iter().enumerate() {
        if let Some(msg) = evaluate_one(body, rule.kind, &rule.value, i + 1) {
            return Some(msg);
        }
    }
    None
}

fn evaluate_one(body: &str, kind: CheckpointKind, expected: &str, index: usize) -> Option<String> {
    let label = format!("检查点 #{}", index);

    match kind {
        CheckpointKind::Contains => {
            if body.contains(expected) {
                None
            } else {
                Some(format!(
                    "{} [包含] 响应体未包含: {}",
                    label,
                    truncate_display(expected)
                ))
            }
        }
        CheckpointKind::Equals => {
            if body == expected {
                None
            } else {
                Some(format!(
                    "{} [等于] 响应体与预期不一致（长度 {} vs {}）",
                    label,
                    body.len(),
                    expected.len()
                ))
            }
        }
        CheckpointKind::NotContains => {
            if !body.contains(expected) {
                None
            } else {
                Some(format!(
                    "{} [不包含] 响应体不应包含: {}",
                    label,
                    truncate_display(expected)
                ))
            }
        }
        CheckpointKind::Regex => match Regex::new(expected) {
            Ok(re) => {
                if re.is_match(body) {
                    None
                } else {
                    Some(format!("{} [正则匹配] 未匹配模式: {}", label, expected))
                }
            }
            Err(e) => Some(format!("{} [正则匹配] 无效正则: {}", label, e)),
        },
        CheckpointKind::NotRegex => match Regex::new(expected) {
            Ok(re) => {
                if !re.is_match(body) {
                    None
                } else {
                    Some(format!("{} [正则不匹配] 不应匹配模式: {}", label, expected))
                }
            }
            Err(e) => Some(format!("{} [正则不匹配] 无效正则: {}", label, e)),
        },
    }
}

fn truncate_display(s: &str) -> String {
    if s.len() <= 80 {
        s.to_string()
    } else {
        format!("{}…", &s[..80])
    }
}
