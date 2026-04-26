use serde::{Deserialize, Serialize};

use crate::clients::grpc::v1::workflows::{
    ConcurrencyLimitStrategy as ProtoConcurrencyLimitStrategy, Concurrency as ProtoConcurrency,
    CreateTaskRateLimit, RateLimitDuration as ProtoRateLimitDuration,
};

/// Strategy to apply when the concurrency limit is reached.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConcurrencyLimitStrategy {
    /// Cancel the currently in-progress run when a new one arrives.
    CancelInProgress,
    /// Distribute runs across concurrency keys in a round-robin fashion.
    GroupRoundRobin,
    /// Cancel the newest run when the limit is reached.
    CancelNewest,
}

impl ConcurrencyLimitStrategy {
    pub(crate) fn to_proto(&self) -> i32 {
        match self {
            ConcurrencyLimitStrategy::CancelInProgress => {
                ProtoConcurrencyLimitStrategy::CancelInProgress as i32
            }
            ConcurrencyLimitStrategy::GroupRoundRobin => {
                ProtoConcurrencyLimitStrategy::GroupRoundRobin as i32
            }
            ConcurrencyLimitStrategy::CancelNewest => {
                ProtoConcurrencyLimitStrategy::CancelNewest as i32
            }
        }
    }
}

/// A concurrency expression controlling how many workflow runs can execute
/// concurrently for a given dynamic key.
///
/// Concurrency is set at the **workflow level**.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConcurrencyExpression {
    /// A CEL expression evaluated against task input to derive the concurrency key
    /// (e.g. `"input.provider_id"`).
    pub expression: String,
    /// Maximum number of concurrent runs sharing the same key.
    pub max_runs: i32,
    /// Strategy to apply when the limit is reached.
    pub limit_strategy: ConcurrencyLimitStrategy,
}

impl ConcurrencyExpression {
    pub(crate) fn to_proto(&self) -> ProtoConcurrency {
        ProtoConcurrency {
            expression: self.expression.clone(),
            max_runs: Some(self.max_runs),
            limit_strategy: Some(self.limit_strategy.to_proto()),
        }
    }
}

/// Duration window for dynamic rate limits.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimitDuration {
    Second,
    Minute,
    Hour,
    Day,
    Week,
    Month,
    Year,
}

impl RateLimitDuration {
    pub(crate) fn to_proto(&self) -> i32 {
        match self {
            RateLimitDuration::Second => ProtoRateLimitDuration::Second as i32,
            RateLimitDuration::Minute => ProtoRateLimitDuration::Minute as i32,
            RateLimitDuration::Hour => ProtoRateLimitDuration::Hour as i32,
            RateLimitDuration::Day => ProtoRateLimitDuration::Day as i32,
            RateLimitDuration::Week => ProtoRateLimitDuration::Week as i32,
            RateLimitDuration::Month => ProtoRateLimitDuration::Month as i32,
            RateLimitDuration::Year => ProtoRateLimitDuration::Year as i32,
        }
    }
}

/// A rate limit applied at the **task/step level**.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RateLimit {
    /// A static rate limit with a fixed key known at registration time.
    Static {
        /// The rate limit key (must already be configured on the Hatchet server).
        key: String,
        /// Number of rate limit units this task consumes per execution.
        units: i32,
    },
    /// A dynamic rate limit whose key is derived at runtime via a CEL expression.
    Dynamic {
        /// The base rate limit key string.
        key: String,
        /// A CEL expression evaluated against task input to determine the rate limit key.
        key_expr: String,
        /// Number of rate limit units this task consumes per execution.
        units: i32,
        /// Optional CEL expression evaluated against task input to determine the number of units
        /// consumed per execution (e.g. `"input.weight"`). When `None`, `units` is used instead.
        units_expr: Option<String>,
        /// Total number of rate limit units available in the window.
        limit: i32,
        /// The duration window for the rate limit.
        duration: RateLimitDuration,
    },
}

impl RateLimit {
    pub(crate) fn to_proto(&self) -> CreateTaskRateLimit {
        match self {
            RateLimit::Static { key, units } => CreateTaskRateLimit {
                key: key.clone(),
                units: Some(*units),
                key_expr: None,
                units_expr: None,
                limit_values_expr: None,
                duration: None,
            },
            RateLimit::Dynamic {
                key,
                key_expr,
                units,
                units_expr,
                limit,
                duration,
            } => CreateTaskRateLimit {
                key: key.clone(),
                units: Some(*units),
                key_expr: Some(key_expr.clone()),
                units_expr: units_expr.clone(),
                limit_values_expr: Some(limit.to_string()),
                duration: Some(duration.to_proto()),
            },
        }
    }
}
