use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct GetWorkflowRunResponse {
    pub tasks: Vec<Task>,
    pub run: Run,

    pub output: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct Task {
    pub status: WorkflowStatus,
    #[serde(rename = "errorMessage")]
    pub error_message: String,
    pub output: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct Run {
    pub status: WorkflowStatus,
    #[serde(rename = "errorMessage")]
    pub error_message: String,
    pub output: Option<Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum WorkflowStatus {
    Running,
    Failed,
    Completed,
    Queued,
    Cancelled,
    #[serde(other)]
    Unknown,
}
