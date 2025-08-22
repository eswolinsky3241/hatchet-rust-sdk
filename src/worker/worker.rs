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
use crate::workflows::context::Context;

pub struct Worker<C> {
    pub name: String,
    max_runs: i32,
    pub client: C,
    tasks: Arc<Mutex<HashMap<String, Arc<ErasedTaskFn<C>>>>>,
    workflows: Vec<crate::grpc::v1::workflows::CreateWorkflowVersionRequest>,
}

impl<C> Worker<C>
where
    C: HatchetClientTrait,
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
        workflow: crate::workflows::workflow::Workflow<I, O, C>,
    ) -> Self
    where
        I: Serialize + Send + Sync,
        O: DeserializeOwned + Send + Sync,
    {
        self.workflows.push(workflow.to_proto());

        for task in workflow.erased_tasks {
            let fully_qualified_name = format!("{}:{}", workflow.name, task.name);
            let task_fn: Arc<ErasedTaskFn<C>> = Arc::new(Box::new(
                move |input: serde_json::Value, ctx: Context<C>| task.function.call(input, ctx),
            ));
            self.tasks
                .lock()
                .unwrap()
                .insert(fully_qualified_name, task_fn);
        }
        self
    }

    pub async fn register_workflows(&mut self) {
        for workflow in &self.workflows {
            self.client
                .put_workflow(
                    &workflow.name,
                    workflow.tasks.clone(),
                    workflow.event_triggers.clone(),
                )
                .await
                .unwrap();
        }
    }

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
                .await
                .unwrap();
        });

        tokio::try_join!(
            async {
                loop {
                    self.client.heartbeat(&worker_id).await?;
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
        client: &mut C,
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
