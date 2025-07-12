use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct GetWorkflowRunResponse {
    pub tasks: Vec<Task>,
}

#[derive(Debug, Deserialize)]
pub struct Task {
    pub status: String,
    pub output: Option<Value>,
}
