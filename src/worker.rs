use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use dispatcher::dispatcher_client::DispatcherClient;
use dispatcher::{HeartbeatRequest, WorkerListenRequest, WorkerRegisterRequest};
use prost_types::Timestamp;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tonic::Request;

use crate::client::HatchetClient;
use crate::error::HatchetError;
use crate::task::Task;
use crate::workflow::Workflow;

pub mod dispatcher {
    tonic::include_proto!("_");
}

pub struct WorkerState<I, O> {
    pub tasks: Arc<Mutex<HashMap<String, (JoinHandle<()>, CancellationToken)>>>,
    pub handlers: HashMap<String, Arc<dyn Task<I, O>>>,
}

use std::time::{SystemTime, UNIX_EPOCH};
pub struct Worker<'a, I, O> {
    pub name: String,
    pub id: String,
    pub client: &'a HatchetClient,
    pub state: Arc<WorkerState<I, O>>,
}

impl<'a, I, O> Worker<'a, I, O>
where
    I: serde::de::DeserializeOwned + Send + 'static,
    O: serde::Serialize + std::fmt::Debug + Send + 'static,
{
    pub async fn new<T>(
        client: &'a HatchetClient,
        name: String,
        task: T,
    ) -> Result<Self, HatchetError>
    where
        T: Task<I, O> + 'static,
        I: DeserializeOwned + Send + 'static,
        O: Serialize + Send + 'static,
    {
        let actions = vec![task.name().to_string()];
        let worker_id = Self::register_worker(client, &name, actions).await?;

        // Build the handler registry
        let mut handlers: HashMap<String, Arc<dyn Task<I, O>>> = HashMap::new();
        handlers.insert(task.name().to_string(), Arc::new(task));

        let state = Arc::new(WorkerState {
            tasks: Arc::new(Mutex::new(HashMap::new())),
            handlers,
        });

        Ok(Self {
            name,
            id: worker_id,
            client,
            state,
        })
    }

    pub fn register_workflow(_workflow: Workflow<I, O>) -> Result<(), HatchetError> {
        Ok(())
    }

    pub async fn start(&self) -> Result<(), HatchetError> {
        println!("{}", proto_timestamp_now());
        let heartbeat_worker = async {
            loop {
                self.heartbeat().await?;
                tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
            }
            #[allow(unreachable_code)]
            Ok::<(), HatchetError>(())
        };

        let listen_worker = self.listen();

        tokio::try_join!(heartbeat_worker, listen_worker)?;

        Ok(())
    }

    pub async fn heartbeat(&self) -> Result<(), HatchetError> {
        let request = Request::new(HeartbeatRequest {
            worker_id: self.id.clone(),
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

    pub async fn listen(&self) -> Result<(), HatchetError> {
        let request = Request::new(WorkerListenRequest {
            worker_id: self.id.clone(),
        });

        let response = self
            .client
            .grpc_stream(request, |channel, request| async move {
                let mut client = DispatcherClient::new(channel);
                client.listen_v2(request).await
            })
            .await?;
        let mut response = response.into_inner();
        loop {
            match response.message().await {
                Ok(message) => match message {
                    Some(message) => {
                        println!("{:?}", message);
                        match message.action_type().as_str_name() {
                            "START_STEP_RUN" => {
                                let handler = self.state.handlers[&message.action_id].clone();
                                let payload = message.action_payload.clone();
                                let step_id = message.step_run_id.clone();

                                let handle = tokio::spawn(async move {
                                    println!("[{step_id}] Received task. Starting handler...");

                                    let raw_json: serde_json::Value =
                                        serde_json::from_str(&payload)
                                            .expect("could not parse action_payload as JSON");

                                    let input_value = raw_json
                                        .get("input")
                                        .cloned()
                                        .expect("missing `input` field in action_payload");

                                    let input = serde_json::from_str(&input_value.to_string())
                                        .expect("could not deserialize task input");

                                    let result = handler.run(input).await;

                                    println!(
                                        "[{step_id}] Handler completed with result: {:?}",
                                        result
                                    );
                                });

                                // You don't need to store `handle` for now since you’re not tracking or cancelling
                            }
                            "CANCEL_STEP_RUN" => println!("CANCELING"),
                            _ => println!("GOT SOMETHING ELSE"),
                        };
                    }
                    None => return Ok(()),
                },
                Err(e) => println!("{e}"),
            };
        }
    }

    async fn register_worker(
        client: &HatchetClient,
        name: &str,
        actions: Vec<String>,
    ) -> Result<String, HatchetError> {
        let request = Request::new(WorkerRegisterRequest {
            worker_name: name.to_string(),
            actions: actions,
            services: vec![],
            max_runs: Some(5),
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
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    Timestamp {
        seconds: now.as_secs() as i64,
        nanos: now.subsec_nanos() as i32,
    }
}
