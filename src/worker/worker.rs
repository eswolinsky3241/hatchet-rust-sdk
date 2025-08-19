use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::mpsc;

use crate::client::HatchetClientTrait;
use crate::error::HatchetError;
use crate::grpc::v0::dispatcher;
use crate::grpc::v0::dispatcher::WorkerRegisterRequest;
use crate::worker::action_listener::ActionListener;
use crate::worker::types::ErasedTaskFn;
use crate::workflows::context::HatchetContextTrait;

pub struct Worker<C, X> {
    pub name: String,
    max_runs: i32,
    pub client: C,
    tasks: Arc<Mutex<HashMap<String, Arc<ErasedTaskFn<X>>>>>,
    workflows: Vec<crate::grpc::v1::workflows::CreateWorkflowVersionRequest>,
}

impl<C, X> Worker<C, X>
where
    C: HatchetClientTrait,
    X: HatchetContextTrait,
{
    pub fn new(name: &str, client: C, max_runs: i32) -> Result<Self, HatchetError> {
        Ok(Self {
            name: name.to_string(),
            max_runs,
            client,
            tasks: Arc::new(Mutex::new(HashMap::new())),
            workflows: vec![],
        })
    }

    pub fn add_workflow<I, O>(
        mut self,
        workflow: crate::workflows::workflow::Workflow<I, O, C, X>,
    ) -> Self
    where
        I: Serialize + Send + Sync,
        O: DeserializeOwned + Send + Sync,
    {
        self.workflows.push(workflow.to_proto());

        for task in &workflow.erased_tasks {
            let fully_qualified_name = format!("{}:{}", workflow.name, task.name);
            let task_function = task.function.clone();
            let task_fn = Arc::new(Box::new(move |input: serde_json::Value, ctx: X| {
                task_function.call(input, ctx)
            }) as ErasedTaskFn<X>);
            self.tasks
                .lock()
                .unwrap()
                .insert(fully_qualified_name, task_fn);
        }
        self
    }

    pub async fn register_workflows(&mut self) {
        for workflow in &self.workflows {
            let workflow = crate::grpc::v1::workflows::CreateWorkflowVersionRequest {
                name: workflow.name.clone(),
                description: workflow.description.clone(),
                version: workflow.version.clone(),
                event_triggers: workflow.event_triggers.clone(),
                cron_triggers: workflow.cron_triggers.clone(),
                tasks: workflow.tasks.clone(),
                concurrency: None,
                cron_input: None,
                on_failure_task: None,
                sticky: None,
                default_priority: None,
                concurrency_arr: vec![],
                default_filters: workflow.default_filters.clone(),
            };
            self.client.put_workflow(workflow).await.unwrap();
        }
    }

    pub async fn start(&mut self, context: X) -> Result<(), HatchetError> {
        let mut actions = vec![];
        for workflow in &self.workflows {
            for task in &workflow.tasks {
                actions.push(task.action.clone());
            }
        }
        let worker_id = Arc::new(
            Self::register_worker(self.client.clone(), &self.name, actions, self.max_runs).await?,
        );
        self.register_workflows().await;

        let (tx, mut rx) = mpsc::channel::<dispatcher::AssignedAction>(self.max_runs as usize);

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
                .listen(worker_id_clone, tx)
                .await
                .unwrap();
        });

        let worker_id_clone = worker_id.clone();
        tokio::try_join!(
            async {
                loop {
                    self.heartbeat(worker_id_clone.clone()).await?;
                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                }
                #[allow(unreachable_code)]
                Ok::<(), HatchetError>(())
            },
            async {
                while let Some(task) = rx.recv().await {
                    let context_clone = context.clone();
                    dispatcher
                        .lock()
                        .await
                        .dispatch(worker_id.clone(), task, context_clone)
                        .await?
                }
                Ok(())
            }
        )?;

        Ok(())
    }

    async fn heartbeat(&mut self, worker_id: Arc<String>) -> Result<(), HatchetError> {
        self.client.heartbeat(&worker_id).await?;

        Ok(())
    }

    async fn register_worker(
        mut client: C,
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

        let response = client.register_worker(registration).await?;

        Ok(response.worker_id)
    }
}
