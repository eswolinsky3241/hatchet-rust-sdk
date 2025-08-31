use crate::clients::grpc::v1::workflows::CreateTaskOpts;
use crate::context::Context;
use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

pub type TaskResult = Pin<Box<dyn Future<Output = Result<serde_json::Value, TaskError>> + Send>>;

#[derive(Debug)]
pub enum TaskError {
    InputDeserialization(serde_json::Error),
    OutputSerialization(serde_json::Error),
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

pub trait ExecutableTask: Send + Sync + dyn_clone::DynClone {
    fn execute(&self, input: serde_json::Value, ctx: Context) -> TaskResult;
    fn name(&self) -> &str;
}

dyn_clone::clone_trait_object!(ExecutableTask);

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
    pub fn new<F, Fut>(name: impl Into<String>, handler: F) -> Self
    where
        F: FnOnce(I, Context) -> Fut + Send + Sync + Clone + 'static,
        Fut: Future<Output = Result<O, E>> + Send + 'static,
    {
        let name = name.into();
        let handler = Arc::new(move |input: I, ctx: Context| {
            let handler_clone = handler.clone();
            Box::pin(handler_clone(input, ctx))
                as Pin<Box<dyn Future<Output = Result<O, E>> + Send>>
        });

        Self {
            name,
            handler,
            parents: vec![],
        }
    }

    pub fn add_parent<J, P, F>(mut self, parent: &Task<J, P, F>) -> Self {
        self.parents.push(parent.name.clone());
        self
    }

    pub fn into_executable(self) -> Box<dyn ExecutableTask> {
        let handler = self.handler;
        let name = self.name;

        Box::new(TypeErasedTask {
            name: name.clone(),
            handler: Arc::new(
                move |input: serde_json::Value, ctx: Context| -> TaskResult {
                    let handler = handler.clone();
                    Box::pin(async move {
                        let typed_input: I = serde_json::from_value(input)
                            .map_err(TaskError::InputDeserialization)?;

                        let result = handler(typed_input, ctx)
                            .await
                            .map_err(|e| TaskError::Execution(e.into()))?;

                        serde_json::to_value(result).map_err(TaskError::OutputSerialization)
                    }) as TaskResult
                },
            ),
        })
    }

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

#[derive(Clone)]
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
