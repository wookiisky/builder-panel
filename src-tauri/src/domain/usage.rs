//! 用量和领域时间模型。

use std::fmt::{Display, Formatter};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Unix 毫秒时间戳。
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct UnixMillis {
    /// Unix epoch 起算的毫秒数。
    pub value: u64,
}

impl UnixMillis {
    /// 创建 Unix 毫秒时间戳。
    pub fn new(value: u64) -> Self {
        Self { value }
    }
}

/// 用量数字校验错误。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UsageAmountError {
    /// 用量数字不是有限数字。
    NotFinite,
    /// 用量数字为负数。
    Negative,
}

impl Display for UsageAmountError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFinite => formatter.write_str("用量数字必须是有限数字"),
            Self::Negative => formatter.write_str("用量数字不能为负数"),
        }
    }
}

/// 已验证可展示的非负有限用量数字。
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct UsageAmount {
    value: f64,
}

impl UsageAmount {
    /// 创建已验证可展示的用量数字。
    pub fn new(value: f64) -> Result<Self, UsageAmountError> {
        if !value.is_finite() {
            return Err(UsageAmountError::NotFinite);
        }

        if value < 0.0 {
            return Err(UsageAmountError::Negative);
        }

        Ok(Self { value })
    }

    /// 返回内部数字。
    pub fn value(&self) -> f64 {
        self.value
    }
}

impl Serialize for UsageAmount {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_f64(self.value)
    }
}

impl<'de> Deserialize<'de> for UsageAmount {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = f64::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// 已验证用量数字。
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct VerifiedUsageValue {
    /// 展示用数字。
    pub value: UsageAmount,
    /// 可选单位。
    pub unit: Option<String>,
    /// 数据来源标签。
    pub source_label: String,
    /// 来源更新时间。
    pub updated_at: Option<UnixMillis>,
}

/// 用量可用性。
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UsageValue {
    /// 已验证可展示。
    Verified(VerifiedUsageValue),
    /// 来源不可用或未验证。
    Unavailable,
}

impl UsageValue {
    /// 返回 UI 展示标签。
    pub fn display_label(&self) -> String {
        match self {
            Self::Verified(verified) => {
                let mut label = trim_float_label(verified.value.value());
                if let Some(unit) = &verified.unit {
                    label.push(' ');
                    label.push_str(unit);
                }
                label
            }
            Self::Unavailable => "--".to_string(),
        }
    }

    /// 返回用量来源标签。
    pub fn source_label(&self) -> Option<&str> {
        match self {
            Self::Verified(verified) => Some(verified.source_label.as_str()),
            Self::Unavailable => None,
        }
    }
}

/// 会话或账号上下文用量。
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct UsageSnapshot {
    /// 5 小时窗口用量。
    pub usage_5h: UsageValue,
    /// 周用量。
    pub usage_weekly: UsageValue,
}

impl UsageSnapshot {
    /// 创建全部不可用的用量快照。
    pub fn unavailable() -> Self {
        Self {
            usage_5h: UsageValue::Unavailable,
            usage_weekly: UsageValue::Unavailable,
        }
    }
}

/// 去掉整数浮点数的尾部小数。
fn trim_float_label(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        UnixMillis, UsageAmount, UsageAmountError, UsageSnapshot, UsageValue, VerifiedUsageValue,
    };

    #[test]
    fn unavailable_usage_uses_placeholder_label() {
        assert_eq!(UsageValue::Unavailable.display_label(), "--");
        assert_eq!(UsageValue::Unavailable.source_label(), None);
    }

    #[test]
    fn verified_usage_keeps_number_unit_and_source() {
        let usage = UsageValue::Verified(VerifiedUsageValue {
            value: UsageAmount::new(42.0).expect("valid usage amount"),
            unit: Some("tokens".to_string()),
            source_label: "Codex /status".to_string(),
            updated_at: Some(UnixMillis::new(1000)),
        });

        assert_eq!(usage.display_label(), "42 tokens");
        assert_eq!(usage.source_label(), Some("Codex /status"));
    }

    #[test]
    fn usage_snapshot_defaults_to_unavailable() {
        let snapshot = UsageSnapshot::unavailable();

        assert_eq!(snapshot.usage_5h, UsageValue::Unavailable);
        assert_eq!(snapshot.usage_weekly, UsageValue::Unavailable);
    }

    #[test]
    fn usage_amount_rejects_negative_and_non_finite_values() {
        assert_eq!(UsageAmount::new(-1.0), Err(UsageAmountError::Negative));
        assert_eq!(UsageAmount::new(f64::NAN), Err(UsageAmountError::NotFinite));
        assert_eq!(
            UsageAmount::new(f64::INFINITY),
            Err(UsageAmountError::NotFinite)
        );
    }

    #[test]
    fn usage_amount_deserialization_rejects_invalid_value() {
        let result = serde_json::from_str::<UsageAmount>("-1");

        assert!(result.is_err());
    }
}
