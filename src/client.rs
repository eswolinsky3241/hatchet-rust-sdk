use dispatcher::WorkerRegisterRequest;
use dispatcher::dispatcher_client::DispatcherClient;
use serde::Serialize;
use tonic::metadata::MetadataValue;
use tonic::transport::{Channel, ClientTlsConfig};
use workflows::TriggerWorkflowRequest;
use workflows::workflow_service_client::WorkflowServiceClient;

use crate::config::HatchetConfig;
use crate::error::HatchetError;
use crate::worker::Worker;
use crate::workflow::RunId;

pub mod workflows {
    tonic::include_proto!("_");
}
pub mod dispatcher {
    tonic::include_proto!("_");
}

#[derive(Debug)]
pub struct HatchetClient {
    config: HatchetConfig,
}

impl HatchetClient {
    pub async fn new(config: HatchetConfig) -> Result<Self, HatchetError> {
        Ok(Self { config })
    }

    pub fn from_env() -> Result<Self, HatchetError> {
        Ok(Self {
            config: HatchetConfig::from_env()?,
        })
    }

    pub async fn register_worker(&self, name: &str) -> Result<String, HatchetError> {
        let channel = self.create_channel().await?;
        let mut client = DispatcherClient::new(channel);

        let mut request = tonic::Request::new(WorkerRegisterRequest {
            worker_name: name.to_string(),
            actions: vec![],
            services: vec![],
            max_runs: None,
            labels: std::collections::HashMap::new(),
            webhook_id: None,
            runtime_info: None,
        });

        self.add_auth_header(&mut request)?;

        let response = client
            .register(request)
            .await
            .map_err(HatchetError::GrpcCall)?;

        Ok(response.into_inner().worker_id)
    }

    pub async fn heartbeat(&self, worker_id: &str) -> Result<(), HatchetError> {
        let channel = self.create_channel().await?;
        let mut client = DispatcherClient::new(channel);

        let mut request = tonic::Request::new(dispatcher::HeartbeatRequest {
            worker_id: worker_id.to_string(),
            heartbeat_at: None,
        });

        self.add_auth_header(&mut request)?;

        client
            .heartbeat(request)
            .await
            .map_err(HatchetError::GrpcCall)?;

        Ok(())
    }

    pub async fn worker(&self, name: String) -> Worker {
        let worker_id = self.register_worker(&name).await.unwrap();
        let worker = Worker {
            client: self,
            id: worker_id,
            name: name,
        };
        worker
    }

    pub async fn trigger_workflow<I>(
        &self,
        task_name: &str,
        input: I,
        options: crate::workflow::TriggerWorkflowOptions,
    ) -> Result<RunId, HatchetError>
    where
        I: Serialize,
    {
        let channel = self.create_channel().await?;
        let mut client = WorkflowServiceClient::new(channel);

        let input_json = serde_json::to_string(&input).map_err(HatchetError::JsonEncode)?;

        let mut request = tonic::Request::new(TriggerWorkflowRequest {
            input: input_json,
            name: task_name.to_string(),
            parent_id: None,
            parent_step_run_id: None,
            child_index: None,
            child_key: None,
            additional_metadata: options.additional_metadata.map(|v| v.to_string()),
            desired_worker_id: options.desired_worker_id,
            priority: None,
        });

        self.add_auth_header(&mut request)?;

        let response = client
            .trigger_workflow(request)
            .await
            .map_err(HatchetError::GrpcCall)?;

        Ok(RunId(response.into_inner().workflow_run_id))
    }

    pub async fn get_workflow(
        &self,
        workflow_run_id: &RunId,
    ) -> Result<crate::models::GetWorkflowRunResponse, HatchetError> {
        let api_client = crate::api::ApiClient::new(
            self.config.server_url.clone(),
            self.config.api_token.clone(),
        );

        api_client
            .get::<crate::models::GetWorkflowRunResponse>(&format!(
                "/api/v1/stable/workflow-runs/{}",
                workflow_run_id
            ))
            .await
    }

    async fn create_channel(&self) -> Result<Channel, HatchetError> {
        let tls_strategy =
            std::env::var("HATCHET_CLIENT_TLS_STRATEGY").unwrap_or("tls".to_string());

        let domain_name = self
            .config
            .grpc_address
            .split(':')
            .next()
            .ok_or(HatchetError::MissingTokenField("grpc_broadcast_address"))?;

        if tls_strategy.to_lowercase() == "none" {
            self.create_insecure_channel().await
        } else {
            self.create_secure_channel(domain_name).await
        }
    }

    async fn create_insecure_channel(&self) -> Result<Channel, HatchetError> {
        Channel::from_shared(format!("http://{}", self.config.grpc_address))
            .map_err(|e| HatchetError::InvalidUri { uri: e.to_string() })?
            .connect()
            .await
            .map_err(HatchetError::GrpcConnect)
    }

    async fn create_secure_channel(&self, domain_name: &str) -> Result<Channel, HatchetError> {
        let tls = ClientTlsConfig::new()
            .domain_name(domain_name)
            .with_native_roots();

        Channel::from_shared(format!("https://{}", self.config.grpc_address))
            .map_err(|e| HatchetError::InvalidUri { uri: e.to_string() })?
            .tls_config(tls)?
            .connect()
            .await
            .map_err(HatchetError::GrpcConnect)
    }

    fn add_auth_header<T>(&self, request: &mut tonic::Request<T>) -> Result<(), HatchetError> {
        let token_header: MetadataValue<_> = format!("Bearer {}", self.config.api_token)
            .parse()
            .map_err(HatchetError::InvalidAuthHeader)?;

        request.metadata_mut().insert("authorization", token_header);
        Ok(())
    }
}
