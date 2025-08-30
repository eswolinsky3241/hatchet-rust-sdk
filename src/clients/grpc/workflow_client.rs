use std::fmt::Debug;

use dyn_clone::DynClone;

use crate::clients::grpc::v0::workflows::workflow_service_client::WorkflowServiceClient;
use crate::clients::grpc::v0::workflows::{TriggerWorkflowRequest, TriggerWorkflowResponse};
use crate::error::HatchetError;
use crate::utils::{EXECUTION_CONTEXT, ExecutionContext};

#[async_trait::async_trait]
pub trait WorkflowClientTrait: DynClone + Debug + Send + Sync + 'static {
    async fn trigger_workflow(
        &mut self,
        trigger_workflow_request: TriggerWorkflowRequest,
    ) -> Result<TriggerWorkflowResponse, HatchetError>;
}

dyn_clone::clone_trait_object!(WorkflowClientTrait);

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
        mut trigger_workflow_request: TriggerWorkflowRequest,
    ) -> Result<TriggerWorkflowResponse, HatchetError> {
        update_task_execution_context(&mut trigger_workflow_request);
        let mut request = tonic::Request::new(trigger_workflow_request);
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        let response = self.client.trigger_workflow(request).await?;
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
