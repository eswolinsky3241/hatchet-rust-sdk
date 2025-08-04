use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::mpsc;
use tonic::Request;

use crate::client::HatchetClient;
use crate::error::HatchetError;
use crate::grpc::dispatcher;
use crate::grpc::dispatcher::dispatcher_client::DispatcherClient;
use crate::grpc::dispatcher::{HeartbeatRequest, WorkerRegisterRequest};
use crate::grpc::workflows::workflow_service_client::WorkflowServiceClient;
use crate::worker::action_listener::ActionListener;
use crate::worker::types::ErasedTaskFn;
use crate::workflows::Context;

pub struct Worker {
    pub name: String,
    worker_id: Option<Arc<String>>,
    max_runs: i32,
    pub client: Arc<HatchetClient>,
    tasks: Arc<Mutex<HashMap<String, Arc<ErasedTaskFn>>>>,
    workflows: Vec<crate::grpc::workflows::CreateWorkflowVersionOpts>,
}

impl Worker {
    pub fn new(name: &str, client: &HatchetClient, max_runs: i32) -> Result<Self, HatchetError> {
        Ok(Self {
            name: name.to_string(),
            worker_id: None,
            max_runs,
            client: Arc::new(client.clone()),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            workflows: vec![],
        })
    }

    pub fn add_workflow<I, O>(
        mut self,
        workflow: crate::workflows::workflow::Workflow<I, O>,
    ) -> Self
    where
        I: Serialize + Send + Sync,
        O: DeserializeOwned + Send + Sync,
    {
        self.workflows.push(workflow.to_proto());
        for task in &workflow.tasks {
            let fully_qualified_name = format!("{}:{}", workflow.name, task.name);
            let task_function = task.function.clone();
            let task_fn = Arc::new(Box::new(move |input: serde_json::Value, ctx: Context| {
                task_function.call(input, ctx)
            }) as ErasedTaskFn);
            self.tasks
                .lock()
                .unwrap()
                .insert(fully_qualified_name, task_fn);
        }
        self
    }

    pub async fn register_workflows(&self) {
        for workflow in &self.workflows {
            let request = Request::new(crate::grpc::workflows::PutWorkflowRequest {
                opts: Some(workflow.clone()),
            });
            self.client
                .grpc_unary(request, |channel, request| async move {
                    let mut client = WorkflowServiceClient::new(channel);
                    client.put_workflow(request).await
                })
                .await
                .unwrap();
        }
    }

    pub async fn start(&mut self) -> Result<(), HatchetError> {
        let mut actions = vec![];
        for workflow in &self.workflows {
            for step in &workflow.jobs[0].steps {
                actions.push(step.action.clone());
            }
        }
        self.worker_id = Some(Arc::new(
            Self::register_worker(&self.client, &self.name, actions, self.max_runs).await?,
        ));
        self.register_workflows().await;

        let (tx, mut rx) = mpsc::channel::<dispatcher::AssignedAction>(100);

        let dispatcher = Arc::new(crate::worker::task_dispatcher::TaskDispatcher {
            registry: self.tasks.clone(),
            client: self.client.clone(),
            task_runs: Arc::new(std::sync::Mutex::new(HashMap::new())),
        });

        let action_listener = Arc::new(ActionListener {
            client: self.client.clone(),
        });
        let worker_id = self.worker_id.clone();
        tokio::spawn(async move {
            action_listener
                .listen(worker_id.unwrap(), tx)
                .await
                .unwrap();
        });

        tokio::try_join!(
            async {
                loop {
                    let worker_id = self.worker_id.clone();
                    self.heartbeat(worker_id.unwrap().to_string()).await?;
                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                }
                #[allow(unreachable_code)]
                Ok::<(), HatchetError>(())
            },
            async {
                while let Some(task) = rx.recv().await {
                    let worker_id = self.worker_id.clone();
                    dispatcher
                        .dispatch(worker_id.unwrap().to_string(), task)
                        .await?
                }
                Ok(())
            }
        )?;

        Ok(())
    }

    async fn heartbeat(&self, worker_id: String) -> Result<(), HatchetError> {
        let request = Request::new(HeartbeatRequest {
            worker_id: worker_id.to_string(),
            heartbeat_at: Some(crate::utils::proto_timestamp_now()?),
        });

        self.client
            .grpc_unary(request, |channel, request| async move {
                let mut client = DispatcherClient::new(channel);
                client.heartbeat(request).await
            })
            .await?;

        Ok(())
    }

    async fn register_worker(
        client: &HatchetClient,
        name: &str,
        actions: Vec<String>,
        max_runs: i32,
    ) -> Result<String, HatchetError> {
        let request = Request::new(WorkerRegisterRequest {
            worker_name: name.to_string(),
            actions: actions,
            services: vec![],
            max_runs: Some(max_runs),
            labels: HashMap::new(),
            webhook_id: None,
            runtime_info: None,
        });

        let response = client
            .grpc_unary(request, |channel, request| async move {
                let mut client = DispatcherClient::new(channel);
                client.register(request).await
            })
            .await?;

        Ok(response.into_inner().worker_id)
    }
}
