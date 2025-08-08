use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(crate) struct GetWorkflowRunResponse {
    pub(crate) tasks: Vec<Task>,
    pub(crate) run: Run,

    pub(crate) output: Option<Value>,
}

struct TaskParent {
    result: serde_json::Value,
}

struct TaskInput {
    parents: HashMap<String, TaskParent>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Task {
    pub(crate) status: WorkflowStatus,
    #[serde(rename = "errorMessage")]
    pub(crate) error_message: String,
    pub(crate) output: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Run {
    pub(crate) status: WorkflowStatus,
    #[serde(rename = "errorMessage")]
    pub(crate) error_message: String,
    pub(crate) output: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub(crate) enum WorkflowStatus {
    Running,
    Failed,
    Completed,
    Queued,
    Cancelled,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Workflow;
