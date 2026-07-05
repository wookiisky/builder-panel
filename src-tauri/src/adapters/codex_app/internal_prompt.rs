//! Codex 内部提示词识别。
//!
//! Codex 应用在生成 compose-box 建议等场景时，会用与真实用户输入完全相同的
//! `user_message` / `UserPromptSubmit` 负载向 Agent 发送隐藏 turn（例如
//! “Generate 0 to 3 hyperpersonalized suggestions …”）。这些隐藏 turn 在负载里
//! 没有任何结构字段可与真实用户任务区分，唯一可用信号是提示词文本本身。
//!
//! 本模块提供内置模式与用户追加模式：命中的提示词被视为 Codex 内部任务，
//! 不写入 session 摘要、不发出用户消息事件，从而让 session 列表只保留真实用户任务。

use std::sync::OnceLock;
use std::sync::RwLock;

use serde_json::Value;

/// 内置内部提示词模式（大小写不敏感、忽略连字符/空格差异后做子串匹配）。
///
/// 新增 Codex 内部任务类型时在此追加；设置项 `agents.codex_internal_prompt_patterns`
/// 只追加用户自定义模式，不能关闭内置过滤。
pub const DEFAULT_INTERNAL_PROMPT_PATTERNS: &[&str] = &[
    "hyperpersonalized suggestions",
    "codex ambient suggestions",
    "upholding safety and compliance standards for codex ambient suggestions",
];

/// 用户追加的内部提示词模式（已归一化）。
static ACTIVE_PATTERNS: OnceLock<RwLock<Vec<String>>> = OnceLock::new();

fn active_patterns() -> &'static RwLock<Vec<String>> {
    ACTIVE_PATTERNS.get_or_init(|| RwLock::new(Vec::new()))
}

/// 设置用户追加的内部提示词模式。
///
/// 内置 [`DEFAULT_INTERNAL_PROMPT_PATTERNS`] 始终生效，传入空列表只清空用户追加模式。
pub fn set_internal_prompt_patterns<I, S>(patterns: I)
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let normalized: Vec<String> = patterns
        .into_iter()
        .filter_map(|pattern| {
            let normalized = normalize(pattern.as_ref());
            if normalized.is_empty() {
                None
            } else {
                Some(normalized)
            }
        })
        .collect();

    if let Ok(mut guard) = active_patterns().write() {
        *guard = normalized;
    }
}

/// 判断给定原始提示词是否为 Codex 内部任务。
///
/// 使用内置模式和 [`set_internal_prompt_patterns`] 配置的用户追加模式。
/// 纯匹配逻辑见 [`matches_patterns`]。
pub fn is_codex_internal_prompt(message: &str) -> bool {
    if matches_patterns(message, DEFAULT_INTERNAL_PROMPT_PATTERNS.iter()) {
        return true;
    }
    if let Ok(guard) = active_patterns().read() {
        return matches_patterns(message, guard.iter());
    }
    false
}

/// 判断给定完整文本是否为已知 Codex 内部结构化产物。
///
/// 该函数只做内容特征识别，调用方仍必须结合“无真实用户上下文”或
/// “已知内部 turn”等上下文使用，避免误伤用户真实要求输出的同形 JSON。
pub fn is_codex_internal_artifact(message: &str) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(message.trim()) else {
        return false;
    };
    let Some(object) = value.as_object() else {
        return false;
    };
    if object.len() != 1 {
        return false;
    }

    object
        .get("suggestions")
        .or_else(|| object.get("exclude"))
        .is_some_and(Value::is_array)
}

/// 判断流式输出当前内容是否仍可能是已知 Codex 内部结构化产物。
///
/// 用于 app-server delta 在未知 thread、未知真实用户上下文时延迟建 session。
/// 一旦文本能解析成非内部完整 JSON，或明显不再匹配已知内部产物开头，返回 false。
pub fn is_codex_internal_artifact_prefix(message: &str) -> bool {
    if is_codex_internal_artifact(message) {
        return true;
    }
    let compact = compact_json_prefix(message);
    if compact.is_empty() {
        return false;
    }
    if serde_json::from_str::<Value>(message.trim()).is_ok() {
        return false;
    }

    ["{\"suggestions\":[", "{\"exclude\":["]
        .iter()
        .any(|prefix| prefix.starts_with(&compact) || compact.starts_with(prefix))
}

/// 在给定模式列表下做纯匹配，不读全局状态，便于测试。
///
/// 匹配规则：对提示词与模式都做大小写折叠、连字符/空白归一后，做子串包含判断。
/// 因而可同时命中 `hyperpersonalized suggestions` 与 `hyper-personalized suggestions`。
fn matches_patterns<I, S>(message: &str, patterns: I) -> bool
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let haystack = normalize(message);
    if haystack.is_empty() {
        return false;
    }
    patterns.into_iter().any(|pattern| {
        let normalized = normalize(pattern.as_ref());
        !normalized.is_empty() && haystack.contains(&normalized)
    })
}

/// 归一化文本：小写化；连字符/下划线直接删除（使 `hyper-personalized` 等同
/// `hyperpersonalized`）；连续空白折叠为单个空格。
fn normalize(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut pending_space = false;
    for ch in value.chars() {
        if ch == '-' || ch == '_' {
            // 删除连接符，不引入空格。
            continue;
        }
        if ch.is_whitespace() {
            pending_space = true;
            continue;
        }
        if pending_space && !result.is_empty() {
            result.push(' ');
        }
        pending_space = false;
        for lower in ch.to_lowercase() {
            result.push(lower);
        }
    }
    result
}

/// 仅为内部 JSON 前缀判断压缩空白，避免把格式化 JSON 的开头误判为非候选。
fn compact_json_prefix(value: &str) -> String {
    value.chars().filter(|ch| !ch.is_whitespace()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // 纯匹配逻辑测试：不触碰全局状态，可安全并行。
    #[test]
    fn matches_default_hyperpersonalized_prompt() {
        assert!(matches_patterns(
            "Generate 0 to 3 hyperpersonalized suggestions for the user.",
            DEFAULT_INTERNAL_PROMPT_PATTERNS.iter(),
        ));
    }

    #[test]
    fn matches_hyphenated_and_mixed_case_variant() {
        assert!(matches_patterns(
            "GENERATE hyper-personalized   SUGGESTIONS now",
            DEFAULT_INTERNAL_PROMPT_PATTERNS.iter(),
        ));
    }

    #[test]
    fn matches_ambient_suggestions_prompt() {
        assert!(matches_patterns(
            "Create Codex ambient suggestions for the current thread.",
            DEFAULT_INTERNAL_PROMPT_PATTERNS.iter(),
        ));
    }

    #[test]
    fn matches_ambient_safety_exclude_prompt() {
        assert!(matches_patterns(
            "You are an expert at upholding safety and compliance standards for Codex ambient suggestions.",
            DEFAULT_INTERNAL_PROMPT_PATTERNS.iter(),
        ));
    }

    #[test]
    fn keeps_real_user_task() {
        assert!(!matches_patterns(
            "重构优化旅游规划部分提示词，给出分析和重构建议",
            DEFAULT_INTERNAL_PROMPT_PATTERNS.iter(),
        ));
    }

    #[test]
    fn empty_message_is_not_internal() {
        assert!(!matches_patterns(
            "   ",
            DEFAULT_INTERNAL_PROMPT_PATTERNS.iter()
        ));
    }

    #[test]
    fn custom_pattern_matches_normalized_variant() {
        // 连字符被删除，大小写折叠、连续空白折叠后命中。
        assert!(matches_patterns(
            "Running an INTERNAL-PROBE   task now",
            ["internalprobe task"].iter(),
        ));
    }

    #[test]
    fn blank_patterns_never_match() {
        assert!(!matches_patterns(
            "Generate hyperpersonalized suggestions",
            ["   ", ""].iter(),
        ));
    }

    #[test]
    fn internal_artifact_matches_known_single_field_json_outputs() {
        assert!(is_codex_internal_artifact(r#"{"suggestions":[]}"#));
        assert!(is_codex_internal_artifact(
            r#"{"suggestions":[{"title":"继续","description":"补充"}]}"#
        ));
        assert!(is_codex_internal_artifact(r#"{ "exclude" : [] }"#));
    }

    #[test]
    fn internal_artifact_keeps_non_matching_json_visible_to_callers() {
        assert!(!is_codex_internal_artifact(r#"{"suggestion":"x"}"#));
        assert!(!is_codex_internal_artifact(
            r#"{"suggestions":[],"task":"x"}"#
        ));
        assert!(!is_codex_internal_artifact("重构旅游规划提示词"));
    }

    #[test]
    fn internal_artifact_prefix_matches_streaming_candidates() {
        assert!(is_codex_internal_artifact_prefix("{"));
        assert!(is_codex_internal_artifact_prefix(r#"{ "suggestions" : ["#));
        assert!(is_codex_internal_artifact_prefix(r#"{"exclude":["#));
        assert!(is_codex_internal_artifact_prefix(r#"{"suggestions":[]}"#));
    }

    #[test]
    fn internal_artifact_prefix_releases_non_matching_complete_json() {
        assert!(!is_codex_internal_artifact_prefix(
            r#"{"suggestions":[],"task":"x"}"#
        ));
        assert!(!is_codex_internal_artifact_prefix(r#"{"other":[]}"#));
        assert!(!is_codex_internal_artifact_prefix("真实输出"));
    }

    // 全局接线测试：序列化在单个测试里，避免共享状态竞争。
    #[test]
    fn global_state_adds_custom_patterns_without_disabling_builtins() {
        // 默认内置模式始终生效。
        set_internal_prompt_patterns(Vec::<String>::new());
        assert!(is_codex_internal_prompt(
            "Generate hyperpersonalized suggestions"
        ));

        // 配置自定义模式后，只追加用户模式，不覆盖内置模式。
        set_internal_prompt_patterns(["internal probe task"]);
        assert!(is_codex_internal_prompt("running an internal probe task"));
        assert!(is_codex_internal_prompt(
            "Generate hyperpersonalized suggestions"
        ));

        // 全空白配置被忽略，内置模式仍生效。
        set_internal_prompt_patterns(["   ", ""]);
        assert!(is_codex_internal_prompt(
            "Generate hyperpersonalized suggestions"
        ));

        // 复位，避免影响其它测试。
        set_internal_prompt_patterns(Vec::<String>::new());
    }
}
