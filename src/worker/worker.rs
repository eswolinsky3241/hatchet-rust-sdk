use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::mpsc;

use crate::clients::grpc::v0::dispatcher;
use crate::clients::grpc::v0::dispatcher::WorkerRegisterRequest;
use crate::clients::hatchet::Hatchet;
use crate::error::HatchetError;
use crate::runnables::*;
use crate::worker::action_listener::ActionListener;

#[derive(typed_builder::TypedBuilder)]
pub struct Worker {
    pub name: String,
    max_runs: i32,
    client: Hatchet,
    #[builder(default = Arc::new(Mutex::new(HashMap::new())))]
    tasks: Arc<Mutex<HashMap<String, Arc<dyn ExecutableTask>>>>,
    #[builder(default = vec![])]
    workflows: Vec<crate::clients::grpc::v1::workflows::CreateWorkflowVersionRequest>,
}

impl Worker {
    pub fn new(name: &str, client: Hatchet, max_runs: i32) -> Result<Self, HatchetError> {
        Ok(Self {
            name: name.to_string(),
            max_runs,
            client,
            tasks: Arc::new(Mutex::new(HashMap::new())),
            workflows: vec![],
        })
    }

    /// Register a workflow with this worker. When the worker starts, it will register the workflow with Hatchet.
    /// Hatchet will then assign runs of the workflow to this worker.
    ///
    /// ```compile_fail
    /// use hatchet_sdk::{Context, Hatchet, EmptyModel};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let hatchet = Hatchet::from_env().await.unwrap();
    ///     let my_task = hatchet.task::<EmptyModel, EmptyModel, MyError>("my-task", |input: EmptyModel, _ctx: Context| async move {
    ///     Ok(EmptyModel)
    /// });
    ///
    /// let my_workflow = hatchet.workflow().name(String::from("my-workflow"))
    ///     .build()
    ///     .unwrap()
    ///     .add_task(my_task)
    ///     .unwrap();
    ///
    ///     let worker = hatchet.worker().name(String::from("my-worker")).build().unwrap();
    ///     worker.add_task_or_workflow(my_workflow);
    /// }
    /// ```
    async fn register_workflows(&mut self) {
        for workflow in &self.workflows {
            self.client
                .admin_client
                .put_workflow(workflow.clone())
                .await
                .unwrap();
        }
    }

    /// Start the worker.
    /// This will register the worker with Hatchet and start listening for assigned tasks.
    /// Use ctrl+c to stop the worker.
    ///
    /// ```no_run
    /// use hatchet_sdk::{Context, Hatchet, EmptyModel, Runnable,Register};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let hatchet = Hatchet::from_env().await.unwrap();
    ///     
    ///     let my_workflow = hatchet.
    ///         workflow::<EmptyModel, EmptyModel>()
    ///         .name(String::from("my-workflow"))
    ///         .build()
    ///         .add_task(hatchet.task("my-task", async move |input: EmptyModel, _ctx: Context| -> anyhow::Result<EmptyModel> {
    ///             Ok(EmptyModel)
    ///         }))
    ///         .unwrap();
    ///
    ///     let mut worker = hatchet.worker()
    ///         .name(String::from("my-worker"))
    ///         .max_runs(5)
    ///         .build()
    ///         .add_task_or_workflow(my_workflow);
    ///
    ///     worker.start().await.unwrap();
    /// }
    /// ```
    pub async fn start(&mut self) -> Result<(), HatchetError> {
        let mut actions = vec![];
        for workflow in &self.workflows {
            for task in &workflow.tasks {
                actions.push(task.action.clone());
            }
        }
        let worker_id = Arc::new(
            Self::register_worker(&mut self.client, &self.name, actions, self.max_runs).await?,
        );
        self.register_workflows().await;

        let (action_tx, mut action_rx) =
            mpsc::channel::<dispatcher::AssignedAction>(self.max_runs as usize);

        let dispatcher = Arc::new(tokio::sync::Mutex::new(
            crate::worker::task_dispatcher::TaskDispatcher {
                registry: self.tasks.clone(),
                client: self.client.clone(),
                task_runs: Arc::new(Mutex::new(HashMap::new())),
            },
        ));

        let action_listener = Arc::new(tokio::sync::Mutex::new(ActionListener::new(
            self.client.clone(),
        )));

        let worker_id_clone = worker_id.clone();
        tokio::spawn(async move {
            action_listener
                .lock()
                .await
                .listen(worker_id_clone, action_tx)
                .await as Result<(), HatchetError>
        });

        tokio::try_join!(
            async {
                loop {
                    self.client.dispatcher_client.heartbeat(&worker_id).await?;
                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                }
                #[allow(unreachable_code)]
                Ok::<(), HatchetError>(())
            },
            async {
                while let Some(task) = action_rx.recv().await {
                    dispatcher
                        .lock()
                        .await
                        .dispatch(worker_id.clone(), task)
                        .await?
                }
                Ok(())
            }
        )?;

        Ok(())
    }

    async fn register_worker(
        client: &mut Hatchet,
        name: &str,
        actions: Vec<String>,
        max_runs: i32,
    ) -> Result<String, HatchetError> {
        let registration = WorkerRegisterRequest {
            worker_name: name.to_string(),
            actions: actions,
            services: vec![],
            max_runs: Some(max_runs),
            labels: HashMap::new(),
            webhook_id: None,
            runtime_info: None,
        };

        let response = client
            .dispatcher_client
            .register_worker(registration)
            .await?;

        Ok(response.worker_id)
    }
}

impl<I, O> Register<Workflow<I, O>, I, O> for Worker
where
    I: Serialize + Send + Sync + 'static,
    O: DeserializeOwned + Send + Sync + 'static,
{
    fn add_task_or_workflow(mut self, workflow: Workflow<I, O>) -> Self {
        self.workflows.push(workflow.to_proto());

        for task in workflow.executable_tasks {
            let fully_qualified_name = format!("{}:{}", workflow.name, task.name());
            self.tasks
                .lock()
                .unwrap()
                .insert(fully_qualified_name, Arc::from(task));
        }
        self
    }
}

impl<I, O, E> Register<Task<I, O, E>, I, O> for Worker
where
    I: DeserializeOwned + Serialize + Send + Sync + 'static,
    O: Serialize + DeserializeOwned + Send + Sync + 'static,
    E: std::error::Error + Send + Sync + 'static,
{
    fn add_task_or_workflow(mut self, workflow: Task<I, O, E>) -> Self {
        let workflow_proto = workflow.to_standalone_workflow_proto();
        self.workflows.push(workflow_proto);

        let fully_qualified_name = format!("{}:{}", workflow.name, workflow.name);
        self.tasks
            .lock()
            .unwrap()
            .insert(fully_qualified_name, Arc::from(workflow.into_executable()));
        self
    }
}

pub trait Register<T, I, O>
where
    I: Serialize + Send + Sync + 'static,
    O: DeserializeOwned + Send + Sync + 'static,
{
    fn add_task_or_workflow(self, workflow: T) -> Self;
}
