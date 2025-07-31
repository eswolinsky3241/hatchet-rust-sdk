use std::cell::RefCell;
use std::time::{SystemTime, UNIX_EPOCH};

use prost_types::Timestamp;

use crate::HatchetError;

pub(crate) fn proto_timestamp_now() -> Result<Timestamp, HatchetError> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?;

    Ok(Timestamp {
        seconds: now.as_secs() as i64,
        nanos: now.subsec_nanos() as i32,
    })
}

#[derive(Clone, Debug)]
pub struct ExecutionContext {
    pub workflow_run_id: String,
    pub step_run_id: String,
    pub worker_id: String,
    pub child_index: i32,
}

tokio::task_local! {
    pub(crate) static EXECUTION_CONTEXT: RefCell<ExecutionContext>;
}
