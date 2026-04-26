#![doc = include_str!("../README.md")]
pub(crate) mod clients;
pub mod config;
pub mod context;
pub mod error;
pub mod runnables;
pub mod utils;
pub mod worker;

pub use clients::hatchet::Hatchet;
pub use clients::rest::features::crons::{
    CreateCronOpts, CronOptions, CronTrigger, CronTriggerList, ListCronsOpts,
};
pub use clients::rest::features::pagination::PaginationResponse;
pub use clients::rest::features::schedules::{
    CreateScheduleOpts, ListSchedulesOpts, ScheduleOptions, ScheduledRun, ScheduledRunList,
};
pub(crate) use clients::{Configuration, GetWorkflowRunResponse, WorkflowStatus};
pub use clients::{CronsClient, RunsClient, SchedulesClient};
pub(crate) use config::{HatchetConfig, TlsStrategy};
pub use context::Context;
pub use error::HatchetError;
pub use runnables::{
    ConcurrencyExpression, ConcurrencyLimitStrategy, RateLimit, RateLimitDuration, Runnable, Task,
    TriggerWorkflowOptionsBuilder, Workflow,
};
pub use utils::EmptyModel;
pub(crate) use utils::{EXECUTION_CONTEXT, ExecutionContext, proto_timestamp_now};
pub use worker::{Register, Worker};

pub mod anyhow {
    pub use anyhow::{Error, Result};
}
pub use {chrono, serde, serde_json, tokio};
