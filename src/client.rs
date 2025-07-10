use crate::config::HatchetConfig;
use crate::error::HatchetError;
use crate::workflow::RunId;
use serde::Serialize;
use tonic::metadata::MetadataValue;
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

    pub async fn run_no_wait<I>(&mut self, task_name: &str, input: I) -> Result<RunId, HatchetError>
    where
        I: Serialize,
    {
        let input_json = serde_json::to_string(&input).map_err(HatchetError::JsonEncode)?;

        let mut grpc_client = WorkflowServiceClient::connect(self.config.grpc_address.clone())
            .await
            .map_err(HatchetError::GrpcConnect)?;

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

        let response = grpc_client
            .trigger_workflow(request)
            .await
            .map_err(HatchetError::GrpcCall)?;

        Ok(RunId(response.into_inner().workflow_run_id))
    }
}
