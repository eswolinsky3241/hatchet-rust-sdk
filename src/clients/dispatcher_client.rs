use std::fmt::Debug;
use tonic::Request;

use crate::error::HatchetError;
use crate::grpc::v0::dispatcher::dispatcher_client::DispatcherClient as DispatcherGrpcClient;
use crate::grpc::v0::dispatcher::{
    AssignedAction, HeartbeatRequest, StepActionEvent, WorkerListenRequest, WorkerRegisterRequest,
    WorkerRegisterResponse,
};

#[async_trait::async_trait]
pub trait DispatcherClientTrait: Clone + Debug + Send + Sync + 'static {
    async fn send_step_action_event(&mut self, event: StepActionEvent) -> Result<(), HatchetError>;

    async fn register_worker(
        &mut self,
        registration: WorkerRegisterRequest,
    ) -> Result<WorkerRegisterResponse, HatchetError>;

    async fn heartbeat(&mut self, worker_id: &str) -> Result<(), HatchetError>;

    async fn listen(
        &mut self,
        worker_id: &str,
    ) -> Result<tonic::Streaming<AssignedAction>, HatchetError>;
}

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

#[async_trait::async_trait]
impl DispatcherClientTrait for DispatcherClient {
    async fn send_step_action_event(&mut self, event: StepActionEvent) -> Result<(), HatchetError> {
        let mut request = Request::new(event);
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        self.client.send_step_action_event(request).await?;
        Ok(())
    }

    async fn register_worker(
        &mut self,
        registration: WorkerRegisterRequest,
    ) -> Result<WorkerRegisterResponse, HatchetError> {
        let mut request = Request::new(registration);
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        Ok(self.client.register(request).await?.into_inner())
    }

    async fn heartbeat(&mut self, worker_id: &str) -> Result<(), HatchetError> {
        let heartbeat = HeartbeatRequest {
            worker_id: worker_id.to_string(),
            heartbeat_at: Some(crate::utils::proto_timestamp_now()?),
        };
        let mut request = Request::new(heartbeat);
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        self.client.heartbeat(request).await?;
        Ok(())
    }

    async fn listen(
        &mut self,
        worker_id: &str,
    ) -> Result<tonic::Streaming<AssignedAction>, HatchetError> {
        let mut request = Request::new(WorkerListenRequest {
            worker_id: worker_id.to_string(),
        });
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        Ok(self.client.listen_v2(request).await?.into_inner())
    }
}
