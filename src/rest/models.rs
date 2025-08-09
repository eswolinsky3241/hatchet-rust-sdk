use std::collections::HashMap;

use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub(crate) struct GetWorkflowRunResponse {
    pub(crate) tasks: Vec<Task>,
    pub(crate) run: Run,
}

#[derive(Debug, Deserialize)]
pub(crate) struct TaskParent(pub(crate) Value);

#[derive(Debug, Deserialize)]
pub(crate) struct TaskInput {
    pub(crate) parents: HashMap<String, TaskParent>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Task {
    #[serde(rename = "errorMessage")]
    pub(crate) output: Option<Value>,
    pub(crate) input: TaskInput,
    #[serde(rename = "taskExternalId")]
    pub(crate) task_external_id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct Run {
    pub(crate) status: WorkflowStatus,
    #[serde(rename = "errorMessage")]
    pub(crate) error_message: String,
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
