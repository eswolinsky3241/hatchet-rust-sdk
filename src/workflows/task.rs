use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::HatchetError;
use crate::client::HatchetClientTrait;
use crate::grpc::v1::workflows::CreateTaskOpts;
use crate::worker::types::{
    self, ErasedHatchetTaskFunction, HatchetTaskFunction, HatchetTaskResult,
};
use crate::workflows::context::Context;

pub struct Task<I, O, E> {
    pub(crate) name: String,
    pub(crate) function: Arc<HatchetTaskFunction<I, O, E>>,
    pub(crate) parents: Vec<String>,
}

// #[derive(Clone)]
pub(crate) struct ErasedTask {
    pub(crate) name: String,
    pub(crate) function: ErasedHatchetTaskFunction,
}

#[async_trait::async_trait]
pub(crate) trait Call<I, O, E> {
    fn call(
        &self,
        input: serde_json::Value,
        ctx: Context,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, E>> + Send>>;
}

#[async_trait::async_trait]
impl<I, O, E> Call<I, O, E> for HatchetTaskFunction<I, O, E>
where
    I: serde::de::DeserializeOwned + Send + 'static,
    O: serde::Serialize + Send + 'static,
    E: std::error::Error + Send + 'static,
{
    fn call(
        &self,
        input: serde_json::Value,
        ctx: Context,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, E>> + Send>> {
        let typed_input: I =
            serde_json::from_value(input).expect("could not deserialize input to expected type");
        let fut = self(typed_input, ctx);
        Box::pin(async move {
            let result = fut.await?;
            let output_json = serde_json::to_value(result).unwrap();
            Ok(output_json)
        })
    }
}

impl<I, O, E> Task<I, O, E>
where
    I: serde::de::DeserializeOwned + Send + 'static,
    O: serde::Serialize + Send + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    pub fn new<F, Fut>(name: &str, f: F) -> Self
    where
        F: Fn(I, Context) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = HatchetTaskResult<O, E>> + Send + 'static,
    {
        let function: Arc<HatchetTaskFunction<I, O, E>> =
            Arc::new(Box::new(move |input: I, ctx: Context| {
                let fut = f(input, ctx);
                Box::pin(fut) as types::HatchetTaskFuture<O, E>
            }));
        Self {
            name: name.to_string(),
            function,
            parents: vec![],
        }
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

    pub fn add_parent<J, P>(mut self, parent: &Task<J, P, E>) -> Self {
        self.parents.push(parent.name.clone());
        self
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

// #[cfg(test)]
// mod test {
//     use super::*;

//     #[test]
//     fn test_task_to_proto() {
//         let task = Task::new(
//             "test-task",
//             |_input: serde_json::Value, _ctx: Context| async move { Ok(()) },
//         );

//         let proto = task.to_proto("test-workflow");
//         assert_eq!(proto.readable_id, "test-task");
//         assert_eq!(proto.action, "test-workflow:test-task");
//         assert_eq!(proto.retries, 0);
//         assert_eq!(proto.rate_limits, vec![]);
//         assert_eq!(proto.worker_labels, std::collections::HashMap::new());
//         assert_eq!(proto.backoff_factor, None);
//         assert_eq!(proto.backoff_max_seconds, None);
//         assert_eq!(proto.concurrency, vec![]);
//         assert_eq!(proto.conditions, None);
//         assert_eq!(proto.schedule_timeout, None);
//     }
// }
