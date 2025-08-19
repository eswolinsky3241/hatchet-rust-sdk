use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use crate::HatchetError;
use crate::grpc::v1::workflows::CreateTaskOpts;
use crate::worker::types::TaskFn;
use crate::workflows::context::HatchetContextTrait;

pub struct Task<I, O, X> {
    pub(crate) name: String,
    pub(crate) function: Arc<TaskFn<I, O, X>>,
    pub(crate) parents: Vec<String>,
}

#[derive(Clone)]
pub(crate) struct ErasedTask<X> {
    pub(crate) name: String,
    pub(crate) function: Arc<dyn ErasedTaskFn<X> + Send + Sync>,
}

pub trait ErasedTaskFn<X>
where
    X: HatchetContextTrait,
{
    fn call(
        &self,
        input: serde_json::Value,
        ctx: X,
    ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, HatchetError>> + Send>>;
}

// Implementation of ErasedTaskFn for TaskFn
impl<I, O, X> ErasedTaskFn<X> for TaskFn<I, O, X>
where
    I: serde::de::DeserializeOwned + Send + 'static,
    O: serde::Serialize + Send + 'static,
    X: HatchetContextTrait,
{
    fn call(
        &self,
        input: serde_json::Value,
        ctx: X,
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

impl<I, O, X> Task<I, O, X>
where
    X: HatchetContextTrait,
{
    pub fn new<F, Fut>(name: &str, f: F) -> Self
    where
        F: Fn(I, X) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O, HatchetError>> + Send + 'static,
    {
        let function = Arc::new(Box::new(move |input: I, ctx: X| {
            let fut = f(input, ctx);
            Box::pin(fut) as Pin<Box<dyn Future<Output = Result<O, HatchetError>> + Send>>
        }) as TaskFn<I, O, X>);
        Self {
            name: name.to_string(),
            function,
            parents: vec![],
        }
    }

    pub fn add_parent<J, P>(mut self, parent: &Task<J, P, X>) -> Self {
        self.parents.push(parent.name.clone());
        self
    }

    pub(crate) fn into_erased(self) -> ErasedTask<X>
    where
        I: serde::de::DeserializeOwned + Send + 'static,
        O: serde::Serialize + Send + 'static,
    {
        ErasedTask {
            name: self.name,
            function: self.function,
        }
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
