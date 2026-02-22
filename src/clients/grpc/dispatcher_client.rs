use super::v0::dispatcher::dispatcher_client::DispatcherClient as DispatcherGrpcClient;
use super::v0::dispatcher::{
    AssignedAction, HeartbeatRequest, StepActionEvent, SubscribeToWorkflowEventsRequest,
    WorkerListenRequest, WorkerRegisterRequest, WorkerRegisterResponse, WorkflowEvent,
};
use crate::HatchetError;
use tonic::Request;
use tonic::Response;

#[derive(Clone, Debug)]
pub(crate) struct DispatcherClient {
    client: DispatcherGrpcClient<tonic::transport::Channel>,
    api_token: String,
}

impl DispatcherClient {
    pub(crate) fn new(channel: tonic::transport::Channel, api_token: String) -> Self {
        let client = DispatcherGrpcClient::new(channel);
        Self { client, api_token }
    }
}

impl DispatcherClient {
    pub async fn send_step_action_event(
        &mut self,
        event: StepActionEvent,
    ) -> Result<(), HatchetError> {
        let mut request = Request::new(event);
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        self.client
            .send_step_action_event(request)
            .await
            .map_err(|e| HatchetError::GrpcErrorStatus(e.message().to_string()))?;
        Ok(())
    }

    pub async fn register_worker(
        &mut self,
        registration: WorkerRegisterRequest,
    ) -> Result<Response<WorkerRegisterResponse>, HatchetError> {
        let mut request = Request::new(registration);
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        self.client
            .register(request)
            .await
            .map_err(|e| HatchetError::GrpcErrorStatus(e.message().to_string()))
    }

    pub async fn heartbeat(&mut self, worker_id: &str) -> Result<(), HatchetError> {
        let heartbeat = HeartbeatRequest {
            worker_id: worker_id.to_string(),
            heartbeat_at: Some(crate::utils::proto_timestamp_now()?),
        };
        let mut request = Request::new(heartbeat);
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        self.client
            .heartbeat(request)
            .await
            .map_err(|e| HatchetError::GrpcErrorStatus(e.message().to_string()))?;
        Ok(())
    }

    pub async fn listen(
        &mut self,
        worker_id: &str,
    ) -> Result<tonic::Streaming<AssignedAction>, HatchetError> {
        let mut request = Request::new(WorkerListenRequest {
            worker_id: worker_id.to_string(),
        });
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        Ok(self
            .client
            .listen_v2(request)
            .await
            .map_err(|e| HatchetError::GrpcErrorStatus(e.message().to_string()))?
            .into_inner())
    }

    pub async fn subscribe_to_workflow_events(
        &mut self,
        workflow_run_id: &str,
    ) -> Result<tonic::Streaming<WorkflowEvent>, HatchetError> {
        let mut request = Request::new(SubscribeToWorkflowEventsRequest {
            workflow_run_id: Some(workflow_run_id.to_string()),
            additional_meta_key: None,
            additional_meta_value: None,
        });
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        Ok(self
            .client
            .subscribe_to_workflow_events(request)
            .await
            .map_err(|e| HatchetError::GrpcErrorStatus(e.message().to_string()))?
            .into_inner())
    }
}
