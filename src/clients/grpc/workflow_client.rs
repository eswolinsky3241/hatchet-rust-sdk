use super::v0::workflows::workflow_service_client::WorkflowServiceClient;
use super::v0::workflows::{TriggerWorkflowRequest, TriggerWorkflowResponse};
use crate::HatchetError;
use crate::{EXECUTION_CONTEXT, ExecutionContext};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Clone, Debug)]
pub(crate) struct WorkflowClient {
    client: Arc<Mutex<WorkflowServiceClient<tonic::transport::Channel>>>,
    api_token: String,
}

impl WorkflowClient {
    pub(crate) fn new(channel: tonic::transport::Channel, api_token: String) -> Self {
        let client = Arc::new(Mutex::new(WorkflowServiceClient::new(channel)));
        Self { client, api_token }
    }
}

impl WorkflowClient {
    pub async fn trigger_workflow(
        &self,
        mut trigger_workflow_request: TriggerWorkflowRequest,
    ) -> Result<TriggerWorkflowResponse, HatchetError> {
        update_task_execution_context(&mut trigger_workflow_request);
        let mut request = tonic::Request::new(trigger_workflow_request);
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        let mut client = self.client.lock().await;
        let response = client.trigger_workflow(request).await?;
        Ok(response.into_inner())
    }
}

fn update_task_execution_context(request: &mut TriggerWorkflowRequest) {
    if let Ok(ctx) = EXECUTION_CONTEXT.try_with(|c| c.clone()) {
        let ctx_inner: ExecutionContext = ctx.into_inner();
        request.child_index = Some(ctx_inner.child_index.clone());
        request.parent_id = Some(ctx_inner.workflow_run_id.clone());
        request.parent_step_run_id = Some(ctx_inner.step_run_id.clone());
        EXECUTION_CONTEXT.with(|ctx| {
            let mut ctx = ctx.borrow_mut();
            ctx.child_index += 1;
        });
    }
}
