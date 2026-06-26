//! Codex 内部提示词识别。
//!
//! Codex 应用在生成 compose-box 建议等场景时，会用与真实用户输入完全相同的
//! `user_message` / `UserPromptSubmit` 负载向 Agent 发送隐藏 turn（例如
//! “Generate 0 to 3 hyperpersonalized suggestions …”）。这些隐藏 turn 在负载里
//! 没有任何结构字段可与真实用户任务区分，唯一可用信号是提示词文本本身。
//!
//! 本模块提供一个可配置的子串模式列表：命中的提示词被视为 Codex 内部任务，
//! 不写入 session 摘要、不发出用户消息事件，从而让 session 列表只保留真实用户任务。

use std::sync::OnceLock;
use std::sync::RwLock;

/// 默认内部提示词模式（大小写不敏感、忽略连字符/空格差异后做子串匹配）。
///
/// 新增 Codex 内部任务类型时，可在此追加模式，或通过设置项
/// `agents.codex_internal_prompt_patterns` 覆盖。
pub const DEFAULT_INTERNAL_PROMPT_PATTERNS: &[&str] = &["hyperpersonalized suggestions"];

/// 当前生效的内部提示词模式（已归一化）。为空表示尚未配置，回退到默认值。
static ACTIVE_PATTERNS: OnceLock<RwLock<Vec<String>>> = OnceLock::new();

fn active_patterns() -> &'static RwLock<Vec<String>> {
    ACTIVE_PATTERNS.get_or_init(|| RwLock::new(Vec::new()))
}

/// 用配置覆盖当前生效的内部提示词模式。
///
/// 传入空列表会清空覆盖并回退到 [`DEFAULT_INTERNAL_PROMPT_PATTERNS`]。
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
/// 使用当前生效的模式列表（[`set_internal_prompt_patterns`] 配置，未配置时回退默认值）。
/// 纯匹配逻辑见 [`matches_patterns`]。
pub fn is_codex_internal_prompt(message: &str) -> bool {
    if let Ok(guard) = active_patterns().read() {
        if !guard.is_empty() {
            return matches_patterns(message, guard.iter());
        }
    }
    matches_patterns(message, DEFAULT_INTERNAL_PROMPT_PATTERNS.iter())
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

    // 全局接线测试：序列化在单个测试里，避免共享状态竞争。
    #[test]
    fn global_state_override_and_fallback() {
        // 默认（未配置）回退到 DEFAULT_INTERNAL_PROMPT_PATTERNS。
        set_internal_prompt_patterns(Vec::<String>::new());
        assert!(is_codex_internal_prompt(
            "Generate hyperpersonalized suggestions"
        ));

        // 配置自定义模式后覆盖默认。
        set_internal_prompt_patterns(["internal probe task"]);
        assert!(is_codex_internal_prompt("running an internal probe task"));
        assert!(!is_codex_internal_prompt(
            "Generate hyperpersonalized suggestions"
        ));

        // 全空白配置被忽略，回退默认。
        set_internal_prompt_patterns(["   ", ""]);
        assert!(is_codex_internal_prompt(
            "Generate hyperpersonalized suggestions"
        ));

        // 复位，避免影响其它测试。
        set_internal_prompt_patterns(Vec::<String>::new());
    }
}
