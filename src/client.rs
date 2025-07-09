use crate::config::HatchetConfig;
use crate::error::HatchetError;
use serde::Serialize;
use tonic::metadata::MetadataValue;
use tonic::transport::Channel;
use workflows::TriggerWorkflowRequest;
use workflows::workflow_service_client::WorkflowServiceClient;

pub mod workflows {
    tonic::include_proto!("_");
}

#[derive(Debug)]
pub struct HatchetClient {
    config: HatchetConfig,
    grpc_client: WorkflowServiceClient<Channel>,
}

impl HatchetClient {
    pub async fn new(config: HatchetConfig) -> Result<Self, HatchetError> {
        let grpc_client = WorkflowServiceClient::connect(config.grpc_address.clone())
            .await
            .map_err(HatchetError::GrpcConnect)?;

        Ok(Self {
            config,
            grpc_client,
        })
    }

    pub async fn run_no_wait<I>(
        &mut self,
        task_name: &str,
        input: I,
    ) -> Result<String, HatchetError>
    where
        I: Serialize,
    {
        let input_json = serde_json::to_string(&input).map_err(HatchetError::JsonEncode)?;

        let mut request = tonic::Request::new(TriggerWorkflowRequest {
            input: input_json,
            name: task_name.to_string(),
            parent_id: None,
            parent_step_run_id: None,
            child_index: None,
            child_key: None,
            additional_metadata: None,
            desired_worker_id: None,
            priority: None,
        });

        let token_header: MetadataValue<_> = format!("Bearer {}", self.config.api_token)
            .parse()
            .map_err(HatchetError::InvalidAuthHeader)?;

        request.metadata_mut().insert("authorization", token_header);

        let response = self
            .grpc_client
            .trigger_workflow(request)
            .await
            .map_err(HatchetError::GrpcCall)?;

        Ok(response.into_inner().workflow_run_id)
    }
}
