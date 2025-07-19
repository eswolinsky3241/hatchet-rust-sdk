use dispatcher::dispatcher_client::DispatcherClient;
use dispatcher::{HeartbeatRequest, WorkerRegisterRequest};
use tonic::Request;

use crate::client::HatchetClient;
use crate::error::HatchetError;
use crate::workflow::Workflow;

pub mod dispatcher {
    tonic::include_proto!("_");
}

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
        loop {
            self.heartbeat().await?;
            tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
        }
    }

    pub async fn heartbeat(&self) -> Result<(), HatchetError> {
        let request = Request::new(HeartbeatRequest {
            worker_id: self.id.clone(),
            heartbeat_at: None,
        });

        self.client
            .grpc_unary_with_auth(request, |channel, request| async move {
                let mut client = DispatcherClient::new(channel);
                client.heartbeat(request).await
            })
            .await?;

        Ok(())
    }

    async fn register_worker(client: &HatchetClient, name: &str) -> Result<String, HatchetError> {
        let request = Request::new(WorkerRegisterRequest {
            worker_name: name.to_string(),
            actions: vec!["simpletask:simpletask".to_string()],
            services: vec![],
            max_runs: Some(5),
            labels: std::collections::HashMap::new(),
            webhook_id: None,
            runtime_info: None,
        });

        let response = client
            .grpc_unary_with_auth(request, |channel, request| async move {
                let mut client = DispatcherClient::new(channel);
                client.register(request).await
            })
            .await?;

        Ok(response.into_inner().worker_id)
    }
}
