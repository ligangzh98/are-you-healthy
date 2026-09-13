use regex::Regex;

#[derive(Debug, Clone)]
pub struct CheckpointRule {
    pub kind: String,
    pub value: String,
}

pub const KINDS: &[&str] = &[
    "contains",
    "equals",
    "not_contains",
    "regex",
    "not_regex",
];

pub fn normalize_kind(kind: &str) -> Option<String> {
    let k = kind.trim().to_lowercase();
    if KINDS.contains(&k.as_str()) {
        Some(k)
    } else {
        None
    }
}

/// 对响应体执行全部检查点，返回第一条失败说明。
pub fn evaluate_all(body: &str, rules: &[CheckpointRule]) -> Option<String> {
    for (i, rule) in rules.iter().enumerate() {
        if let Some(msg) = evaluate_one(body, &rule.kind, &rule.value, i + 1) {
            return Some(msg);
        }
    }
    None
}

fn evaluate_one(body: &str, kind: &str, expected: &str, index: usize) -> Option<String> {
    let kind = kind.to_lowercase();
    let label = format!("检查点 #{}", index);

    match kind.as_str() {
        "contains" => {
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
        "equals" => {
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
        "not_contains" => {
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
        "regex" => match Regex::new(expected) {
            Ok(re) => {
                if re.is_match(body) {
                    None
                } else {
                    Some(format!("{} [正则匹配] 未匹配模式: {}", label, expected))
                }
            }
            Err(e) => Some(format!("{} [正则匹配] 无效正则: {}", label, e)),
        },
        "not_regex" => match Regex::new(expected) {
            Ok(re) => {
                if !re.is_match(body) {
                    None
                } else {
                    Some(format!("{} [正则不匹配] 不应匹配模式: {}", label, expected))
                }
            }
            Err(e) => Some(format!("{} [正则不匹配] 无效正则: {}", label, e)),
        },
        other => Some(format!("{} 未知类型: {}", label, other)),
    }
}

fn truncate_display(s: &str) -> String {
    if s.len() <= 80 {
        s.to_string()
    } else {
        format!("{}…", &s[..80])
    }
}
