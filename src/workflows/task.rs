use crate::HatchetError;
use crate::grpc::workflows::CreateWorkflowStepOpts;
use crate::workflows::task_function::TaskFunction;

pub struct Task<T> {
    pub name: String,
    pub function: T,
    pub parents: Vec<String>,
    workflow: String,
}

impl<T> Task<T> {
    pub fn new(name: String, function: T) -> Self {
        Self {
            name,
            function,
            parents: vec![],
            workflow: "".to_string(),
        }
    }

    pub fn add_parent<U>(mut self, parent: &Self) -> Self {
        self.parents.push(parent.name.clone());
        self
    }

    pub(crate) async fn run<I, O>(&self, input: I, ctx: crate::Context) -> Result<O, HatchetError>
    where
        I: serde::de::DeserializeOwned + Send + Sync + 'static,
        O: serde::Serialize + Send + Sync + 'static,
        T: TaskFunction<I, O>,
    {
        self.function.run(input, ctx).await
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
