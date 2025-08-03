use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::grpc::workflows::CreateWorkflowStepOpts;
use crate::worker::types::TaskFn;
use crate::{Context, HatchetError};

pub struct Task<I, O> {
    pub name: String,
    pub function: Arc<TaskFn<I, O>>,
    pub parents: Vec<String>,
}

#[derive(Clone)]
pub(crate) struct ErasedTask {
    pub(crate) name: String,
    pub(crate) function: Arc<dyn ErasedTaskFn + Send + Sync>,
}

pub trait ErasedTaskFn {
    fn call(
        &self,
        input: serde_json::Value,
        ctx: Context,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, HatchetError>> + Send>>;
}

// Implementation of ErasedTaskFn for TaskFn
impl<I, O> ErasedTaskFn for TaskFn<I, O>
where
    I: serde::de::DeserializeOwned + Send + 'static,
    O: serde::Serialize + Send + 'static,
{
    fn call(
        &self,
        input: serde_json::Value,
        ctx: Context,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, HatchetError>> + Send>> {
        let typed_input: I =
            serde_json::from_value(input).expect("could not deserialize input to expected type");
        let fut = self(typed_input, ctx);
        Box::pin(async move {
            let result = fut.await?;
            let output_json = serde_json::to_value(result)?;
            Ok(output_json)
        })
    }
}

impl<I, O> Task<I, O> {
    pub fn new<F, Fut>(name: &str, f: F) -> Self
    where
        F: Fn(I, Context) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O, HatchetError>> + Send + 'static,
    {
        let function = Arc::new(Box::new(move |input: I, ctx: Context| {
            let fut = f(input, ctx);
            Box::pin(fut) as Pin<Box<dyn Future<Output = Result<O, HatchetError>> + Send>>
        }) as TaskFn<I, O>);
        Self {
            name: name.to_string(),
            function,
            parents: vec![],
        }
    }

    pub fn add_parent<J, P>(mut self, parent: &Task<J, P>) -> Self {
        self.parents.push(parent.name.clone());
        self
    }

    pub(crate) fn into_erased(self) -> ErasedTask
    where
        I: serde::de::DeserializeOwned + Send + 'static,
        O: serde::Serialize + Send + 'static,
    {
        ErasedTask {
            name: self.name,
            function: self.function,
        }
    }

    pub(crate) fn to_proto(&self, workflow_name: &str) -> CreateWorkflowStepOpts {
        CreateWorkflowStepOpts {
            readable_id: self.name.clone(),
            action: format!("{workflow_name}:{}", &self.name),
            timeout: String::from(""),
            inputs: String::from("{{}}"),
            parents: self.parents.clone(),
            user_data: String::from(""),
            retries: 0,
            rate_limits: vec![],
            worker_labels: std::collections::HashMap::new(),
            backoff_factor: None,
            backoff_max_seconds: None,
        }
    }
}
