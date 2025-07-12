use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct GetWorkflowRunResponse {
    pub tasks: Vec<Task>,
    pub run: Run,
}

#[derive(Debug, Deserialize)]
pub struct Task {
    pub status: String,
    pub output: Option<Value>,
}

#[derive(Debug, Deserialize)]
pub struct Run {}
