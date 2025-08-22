use std::fmt::Debug;

use crate::error::HatchetError;
use crate::grpc::v0::workflows::workflow_service_client::WorkflowServiceClient;
use crate::grpc::v0::workflows::{TriggerWorkflowRequest, TriggerWorkflowResponse};

#[async_trait::async_trait]
pub trait WorkflowClientTrait: Clone + Debug + Send + Sync + 'static {
    async fn trigger_workflow(
        &mut self,
        trigger_workflow_request: TriggerWorkflowRequest,
    ) -> Result<TriggerWorkflowResponse, HatchetError>;
}

#[derive(Clone, Debug)]
pub(crate) struct WorkflowClient {
    client: WorkflowServiceClient<tonic::transport::Channel>,
    api_token: String,
}

impl WorkflowClient {
    pub(crate) fn new(channel: tonic::transport::Channel, api_token: String) -> Self {
        let client = WorkflowServiceClient::new(channel);
        Self { client, api_token }
    }
}

#[async_trait::async_trait]
impl WorkflowClientTrait for WorkflowClient {
    async fn trigger_workflow(
        &mut self,
        trigger_workflow_request: TriggerWorkflowRequest,
    ) -> Result<TriggerWorkflowResponse, HatchetError> {
        let mut request = tonic::Request::new(trigger_workflow_request);
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        let response = self.client.trigger_workflow(request).await.unwrap();
        Ok(response.into_inner())
    }
}
