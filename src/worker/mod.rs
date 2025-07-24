pub mod action_listener;

pub mod task_dispatcher;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use dispatcher::dispatcher_client::DispatcherClient;
use dispatcher::{HeartbeatRequest, WorkerRegisterRequest};
use prost_types::Timestamp;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::sync::mpsc;
use tonic::Request;

use crate::client::HatchetClient;
use crate::error::HatchetError;
use crate::task::{ErasedTask, ErasedTaskImpl, Task};
use crate::worker::action_listener::ActionListener;
pub mod dispatcher {
    tonic::include_proto!("_");
}

pub struct Worker {
    pub name: String,
    pub worker_id: Arc<String>,
    pub client: Arc<HatchetClient>,
    pub tasks: Arc<HashMap<String, Arc<dyn ErasedTask>>>,
}

impl Worker {
    pub async fn new<T, I, O>(
        client: HatchetClient,
        name: String,
        task: T,
        max_runs: i32,
    ) -> Result<Self, HatchetError>
    where
        T: Task<I, O> + 'static,
        I: DeserializeOwned + Send + 'static,
        O: Serialize + Send + std::fmt::Debug + 'static,
    {
        let actions = vec![task.name().to_string()];
        let worker_id = Self::register_worker(&client, &name, actions, max_runs).await?;

        let erased = Arc::new(ErasedTaskImpl::new(task));
        let mut tasks: HashMap<String, Arc<dyn ErasedTask>> = HashMap::new();
        tasks.insert(erased.name().to_string(), erased);

        Ok(Self {
            name,
            worker_id: Arc::new(worker_id),
            client: Arc::new(client),
            tasks: Arc::new(tasks),
        })
    }

    pub async fn start(&self) -> Result<(), HatchetError> {
        let (tx, mut rx) = mpsc::channel::<dispatcher::AssignedAction>(100);

        let listener = Arc::new(ActionListener {
            client: self.client.clone(),
        });

        let test_registry = self.tasks.clone();
        let dispatcher = Arc::new(crate::worker::task_dispatcher::TaskDispatcher {
            registry: test_registry,
            client: self.client.clone(),
        });

        let worker_id = self.worker_id.clone();
        tokio::spawn(async move {
            listener.listen(worker_id, tx).await.unwrap();
        });

        let worker_id = self.worker_id.clone();
        tokio::try_join!(
            async {
                loop {
                    self.heartbeat().await?;
                    tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
                }
                #[allow(unreachable_code)]
                Ok::<(), HatchetError>(())
            },
            async {
                while let Some(task) = rx.recv().await {
                    match task.action_type().as_str_name() {
                        "START_STEP_RUN" => {
                            let worker_id = worker_id.clone();
                            let dispatcher = dispatcher.clone();
                            let _ = tokio::spawn(async move {
                                if let Err(e) = dispatcher.dispatch(worker_id, task).await {
                                    eprintln!("Task failed: {e}");
                                }
                            });
                        }
                        "CANCEL_STEP_RUN" => {
                            // Add cancel code here
                        }
                        _ => println!("GOT SOMETHING ELSE"),
                    };
                }
                Ok(())
            }
        )?;

        Ok(())
    }

    pub async fn heartbeat(&self) -> Result<(), HatchetError> {
        let request = Request::new(HeartbeatRequest {
            worker_id: self.worker_id.to_string(),
            heartbeat_at: Some(proto_timestamp_now()),
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

fn proto_timestamp_now() -> Timestamp {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    Timestamp {
        seconds: now.as_secs() as i64,
        nanos: now.subsec_nanos() as i32,
    }
}
