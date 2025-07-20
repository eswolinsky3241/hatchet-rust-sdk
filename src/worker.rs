use std::collections::HashMap;

use dispatcher::dispatcher_client::DispatcherClient;
use dispatcher::{HeartbeatRequest, WorkerListenRequest, WorkerRegisterRequest};
use prost_types::Timestamp;
use tonic::Request;

use crate::client::HatchetClient;
use crate::error::HatchetError;
use crate::workflow::Workflow;

pub mod dispatcher {
    tonic::include_proto!("_");
}
use std::time::{SystemTime, UNIX_EPOCH};
pub struct Worker<'a> {
    pub name: String,
    pub id: String,
    pub client: &'a HatchetClient,
}

impl<'a> Worker<'a> {
    pub async fn new(client: &'a HatchetClient, name: String) -> Result<Self, HatchetError> {
        let worker_id = Self::register_worker(client, &name).await?;
        Ok(Self {
            name,
            id: worker_id,
            client,
        })
    }

    pub fn register_workflow<I, O>(_workflow: Workflow<I, O>) -> Result<(), HatchetError> {
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
                            "START_STEP_RUN" => println!("STARTING {}", message.action_id),
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

    async fn register_worker(client: &HatchetClient, name: &str) -> Result<String, HatchetError> {
        let request = Request::new(WorkerRegisterRequest {
            worker_name: name.to_string(),
            actions: vec!["simpletask:simpletask".to_string()],
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
