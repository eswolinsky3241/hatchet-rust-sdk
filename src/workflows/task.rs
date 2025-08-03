use crate::HatchetError;
use crate::grpc::workflows::CreateWorkflowStepOpts;
use crate::workflows::task_function::TaskFunction;

pub struct Task<F> {
    pub name: String,
    pub function: F,
    pub parents: Vec<String>,
}

impl<F> Task<F> {
    pub fn new<I, O, E, Fut>(name: String, function: F) -> Self
    where
        F: FnOnce(I, crate::Context) -> Fut,
        Fut: Future<Output = Result<O, E>> + Send + 'static,
        O: Send,
        I: serde::Serialize,
        E: std::error::Error,
    {
        Self {
            name: name.to_lowercase(),
            function,
            parents: vec![],
        }
    }

    pub fn add_parent<U>(mut self, parent: &Self) -> Self {
        self.parents.push(parent.name.clone());
        self
    }

    pub(crate) fn to_proto(&self, workflow_name: &str) -> CreateWorkflowStepOpts {
        CreateWorkflowStepOpts {
            readable_id: self.name.clone(),
            action: format!("{workflow_name}:{0}", &self.name),
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
