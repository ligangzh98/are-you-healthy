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

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(kind: CheckpointKind, value: &str) -> CheckpointRule {
        CheckpointRule {
            kind,
            value: value.to_string(),
        }
    }

    #[test]
    fn normalize_kind_accepts_aliases() {
        assert_eq!(normalize_kind("CONTAINS"), Some(CheckpointKind::Contains));
        assert_eq!(normalize_kind(" not_contains "), Some(CheckpointKind::NotContains));
        assert!(normalize_kind("unknown").is_none());
    }

    #[test]
    fn evaluate_all_passes_when_all_rules_ok() {
        let rules = vec![
            rule(CheckpointKind::Contains, "ell"),
            rule(CheckpointKind::Equals, "hello"),
        ];
        assert!(evaluate_all("hello", &rules).is_none());
    }

    #[test]
    fn evaluate_all_returns_first_failure() {
        let rules = vec![
            rule(CheckpointKind::Contains, "missing"),
            rule(CheckpointKind::Equals, "x"),
        ];
        let err = evaluate_all("hello", &rules).unwrap();
        assert!(err.contains("检查点 #1"));
        assert!(err.contains("[包含]"));
    }

    #[test]
    fn equals_requires_exact_body() {
        let rules = vec![rule(CheckpointKind::Equals, "ab")];
        assert!(evaluate_all("ab", &rules).is_none());
        assert!(evaluate_all("abc", &rules).is_some());
    }

    #[test]
    fn not_contains_fails_when_substring_present() {
        let rules = vec![rule(CheckpointKind::NotContains, "err")];
        assert!(evaluate_all("all good", &rules).is_none());
        assert!(evaluate_all("error", &rules).is_some());
    }

    #[test]
    fn regex_rule_and_invalid_pattern() {
        let ok = vec![rule(CheckpointKind::Regex, r"\d+")];
        assert!(evaluate_all("id 42", &ok).is_none());
        assert!(evaluate_all("no digits", &ok).is_some());

        let bad = vec![rule(CheckpointKind::Regex, "[")];
        let err = evaluate_all("x", &bad).unwrap();
        assert!(err.contains("无效正则"));
    }

    #[test]
    fn not_regex_rule() {
        let rules = vec![rule(CheckpointKind::NotRegex, r"error")];
        assert!(evaluate_all("ok", &rules).is_none());
        assert!(evaluate_all("error page", &rules).is_some());
    }
}
