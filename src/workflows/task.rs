use crate::grpc::v1::workflows::CreateTaskOpts;
use crate::workflows::context::Context;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

/// The result type for all erased tasks
pub type TaskResult = Pin<Box<dyn Future<Output = Result<serde_json::Value, TaskError>> + Send>>;

/// Unified error type for task execution
#[derive(Debug)]
pub enum TaskError {
    /// Failed to deserialize input JSON to expected type
    InputDeserialization(serde_json::Error),
    /// Failed to serialize output to JSON
    OutputSerialization(serde_json::Error),
    /// Task execution failed
    Execution(Box<dyn std::error::Error + Send + Sync>),
}

impl std::fmt::Display for TaskError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaskError::InputDeserialization(e) => write!(f, "Failed to deserialize input: {}", e),
            TaskError::OutputSerialization(e) => write!(f, "Failed to serialize output: {}", e),
            TaskError::Execution(e) => write!(f, "Task execution failed: {}", e),
        }
    }
}

impl std::error::Error for TaskError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TaskError::InputDeserialization(e) => Some(e),
            TaskError::OutputSerialization(e) => Some(e),
            TaskError::Execution(e) => Some(e.as_ref()),
        }
    }
}

/// A type-erased task that can be executed with JSON input/output
pub trait ExecutableTask: Send + Sync {
    /// Execute the task with JSON input and return JSON output
    fn execute(&self, input: serde_json::Value, ctx: Context) -> TaskResult;
    /// Get the task name
    fn name(&self) -> &str;
}

/// A typed task that users create
pub struct Task<I, O, E> {
    pub(crate) name: String,
    handler:
        Arc<dyn Fn(I, Context) -> Pin<Box<dyn Future<Output = Result<O, E>> + Send>> + Send + Sync>,
    parents: Vec<String>,
}

impl<I, O, E> Task<I, O, E>
where
    I: for<'de> Deserialize<'de> + Send + 'static,
    O: Serialize + Send + 'static,
    E: Into<Box<dyn std::error::Error + Send + Sync>> + Send + 'static,
{
    /// Create a new task with the given name and handler function
    pub fn new<F, Fut>(name: impl Into<String>, handler: F) -> Self
    where
        F: Fn(I, Context) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O, E>> + Send + 'static,
    {
        let name = name.into();
        let handler = Arc::new(move |input: I, ctx: Context| {
            Box::pin(handler(input, ctx)) as Pin<Box<dyn Future<Output = Result<O, E>> + Send>>
        });

        Self {
            name,
            handler,
            parents: vec![],
        }
    }

    /// Add a parent task dependency
    pub fn add_parent<J, P, F>(mut self, parent: &Task<J, P, F>) -> Self {
        self.parents.push(parent.name.clone());
        self
    }

    /// Convert to a type-erased executable task
    pub fn into_executable(self) -> Box<dyn ExecutableTask> {
        let handler = self.handler;
        let name = self.name;

        Box::new(TypeErasedTask {
            name: name.clone(),
            handler: Arc::new(
                move |input: serde_json::Value, ctx: Context| -> TaskResult {
                    let handler = handler.clone();
                    Box::pin(async move {
                        // Deserialize input with proper error handling
                        let typed_input: I = serde_json::from_value(input)
                            .map_err(TaskError::InputDeserialization)?;

                        // Execute task with proper error handling
                        let result = handler(typed_input, ctx)
                            .await
                            .map_err(|e| TaskError::Execution(e.into()))?;

                        // Serialize output with proper error handling
                        serde_json::to_value(result).map_err(TaskError::OutputSerialization)
                    }) as TaskResult
                },
            ),
        })
    }

    /// Generate protobuf representation for workflow registration
    pub(crate) fn to_proto(&self, workflow_name: &str) -> CreateTaskOpts {
        CreateTaskOpts {
            readable_id: self.name.clone(),
            action: format!("{workflow_name}:{}", &self.name),
            timeout: String::from(""),
            inputs: String::from("{{}}"),
            parents: self.parents.clone(),
            retries: 0,
            rate_limits: vec![],
            worker_labels: std::collections::HashMap::new(),
            backoff_factor: None,
            backoff_max_seconds: None,
            concurrency: vec![],
            conditions: None,
            schedule_timeout: None,
        }
    }
}

/// Internal implementation of ExecutableTask
struct TypeErasedTask {
    name: String,
    handler: Arc<dyn Fn(serde_json::Value, Context) -> TaskResult + Send + Sync>,
}

impl ExecutableTask for TypeErasedTask {
    fn execute(&self, input: serde_json::Value, ctx: Context) -> TaskResult {
        (self.handler)(input, ctx)
    }

    fn name(&self) -> &str {
        &self.name
    }
}

// Backward compatibility types - these will be removed in the next step
pub(crate) struct ErasedTask {
    pub(crate) name: String,
    pub(crate) executable: Box<dyn ExecutableTask>,
}

impl ErasedTask {
    pub fn from_executable(executable: Box<dyn ExecutableTask>) -> Self {
        let name = executable.name().to_string();
        Self { name, executable }
    }
}

// Temporary compatibility - will be removed
impl<I, O, E> Task<I, O, E>
where
    I: for<'de> Deserialize<'de> + Send + 'static,
    O: Serialize + Send + 'static,
    E: Into<Box<dyn std::error::Error + Send + Sync>> + Send + 'static,
{
    pub(crate) fn into_erased(self) -> ErasedTask {
        let executable = self.into_executable();
        ErasedTask::from_executable(executable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_task_execution() {
        let task = Task::new("test-task", |input: i32, _ctx: Context| async move {
            Ok::<i32, std::io::Error>(input * 2)
        });

        let executable = task.into_executable();
        let ctx = Context::new(
            Box::new(crate::client::HatchetClient::from_env().await.unwrap()),
            "test-workflow-run",
            "test-step-run",
        )
        .await;

        let result = executable.execute(serde_json::json!(5), ctx).await.unwrap();
        assert_eq!(result, serde_json::json!(10));
    }
}
