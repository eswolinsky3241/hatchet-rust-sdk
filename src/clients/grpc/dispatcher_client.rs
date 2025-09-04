use std::sync::Arc;
use tokio::sync::Mutex;
use tonic::Request;

use crate::clients::grpc::v0::dispatcher::dispatcher_client::DispatcherClient as DispatcherGrpcClient;
use crate::clients::grpc::v0::dispatcher::{
    AssignedAction, HeartbeatRequest, StepActionEvent, WorkerListenRequest, WorkerRegisterRequest,
    WorkerRegisterResponse,
};
use crate::error::HatchetError;

#[derive(Clone, Debug)]
pub(crate) struct DispatcherClient {
    client: Arc<Mutex<DispatcherGrpcClient<tonic::transport::Channel>>>,
    api_token: String,
}

impl DispatcherClient {
    pub(crate) fn new(channel: tonic::transport::Channel, api_token: String) -> Self {
        let client = Arc::new(Mutex::new(DispatcherGrpcClient::new(channel)));
        Self { client, api_token }
    }
}

impl DispatcherClient {
    pub(crate) async fn send_step_action_event(
        &mut self,
        event: StepActionEvent,
    ) -> Result<(), HatchetError> {
        let mut request = Request::new(event);
        let mut client = self.client.lock().await;
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        client.send_step_action_event(request).await?;
        Ok(())
    }

    pub(crate) async fn register_worker(
        &mut self,
        registration: WorkerRegisterRequest,
    ) -> Result<WorkerRegisterResponse, HatchetError> {
        let mut request = Request::new(registration);
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        let mut client = self.client.lock().await;
        Ok(client.register(request).await?.into_inner())
    }

    pub(crate) async fn heartbeat(&mut self, worker_id: &str) -> Result<(), HatchetError> {
        let heartbeat = HeartbeatRequest {
            worker_id: worker_id.to_string(),
            heartbeat_at: Some(crate::utils::proto_timestamp_now()?),
        };
        let mut request = Request::new(heartbeat);
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        let mut client = self.client.lock().await;
        client.heartbeat(request).await?;
        Ok(())
    }

    pub(crate) async fn listen(
        &mut self,
        worker_id: &str,
    ) -> Result<tonic::Streaming<AssignedAction>, HatchetError> {
        let mut request = Request::new(WorkerListenRequest {
            worker_id: worker_id.to_string(),
        });
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        let mut client = self.client.lock().await;
        Ok(client.listen_v2(request).await?.into_inner())
    }
}
