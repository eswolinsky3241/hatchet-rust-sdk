use crate::config::HatchetConfig;
use crate::error::HatchetError;
use crate::workflow::RunId;
use serde::Serialize;
use tonic::metadata::MetadataValue;
use tonic::transport::{Channel, ClientTlsConfig};
use workflows::TriggerWorkflowRequest;
use workflows::workflow_service_client::WorkflowServiceClient;

pub mod workflows {
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

        let input_json = serde_json::to_string(&input)
            .map_err(HatchetError::JsonEncode)
            .unwrap();

        let mut client = WorkflowServiceClient::new(channel);

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

        let token_header: MetadataValue<_> = format!("Bearer {}", self.config.api_token)
            .parse()
            .map_err(HatchetError::InvalidAuthHeader)
            .unwrap();

        request.metadata_mut().insert("authorization", token_header);

        let response = client
            .trigger_workflow(request)
            .await
            .map_err(HatchetError::GrpcCall)
            .unwrap();

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
}

impl HatchetClient {
    async fn create_channel(&self) -> Result<Channel, HatchetError> {
        let tls_strategy =
            std::env::var("HATCHET_CLIENT_TLS_STRATEGY").unwrap_or_else(|_| "tls".to_string());

        let domain_name = self
            .config
            .grpc_address
            .split(':')
            .next()
            .ok_or(HatchetError::MissingGrpcAddress)?;

        if tls_strategy.to_lowercase() == "none" {
            let channel = Channel::from_shared(format!("http://{}", self.config.grpc_address))
                .unwrap()
                .connect()
                .await
                .map_err(HatchetError::GrpcConnect)?;
            Ok(channel)
        } else {
            let tls = ClientTlsConfig::new()
                .domain_name(domain_name)
                .with_native_roots();

            let channel = Channel::from_shared(format!("https://{}", self.config.grpc_address))
                .unwrap()
                .tls_config(tls)
                .unwrap()
                .connect()
                .await
                .map_err(HatchetError::GrpcConnect)?;
            Ok(channel)
        }
    }
}
