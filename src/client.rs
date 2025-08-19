use tonic::transport::{Channel, ClientTlsConfig};

use crate::clients::{
    AdminClient,
    AdminClientTrait,
    DispatcherClient,
    DispatcherClientTrait,
    EventClient,
    EventClientTrait,
    WorkflowClient,
    WorkflowClientTrait,
};
use crate::config::{HatchetConfig, TlsStrategy};
use crate::error::HatchetError;
use crate::grpc::v0::dispatcher::{
    AssignedAction,
    StepActionEvent,
    WorkerRegisterRequest,
    WorkerRegisterResponse,
};
use crate::grpc::v0::workflows::{TriggerWorkflowRequest, TriggerWorkflowResponse};
use crate::grpc::v1::workflows::CreateWorkflowVersionRequest;

#[async_trait::async_trait]
pub(crate) trait HatchetClientTrait: Clone + Send + Sync + 'static {
    async fn get_workflow_run(
        &self,
        run_id: &str,
    ) -> Result<crate::rest::models::GetWorkflowRunResponse, HatchetError>;

    async fn put_workflow(
        &mut self,
        workflow: CreateWorkflowVersionRequest,
    ) -> Result<(), HatchetError>;

    async fn trigger_workflow(
        &mut self,
        trigger_workflow_request: TriggerWorkflowRequest,
    ) -> Result<TriggerWorkflowResponse, HatchetError>;

    async fn put_log(
        &mut self,
        step_run_id: &str,
        message: String,
    ) -> Result<(), crate::HatchetError>;

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

    async fn api_get<T>(&self, path: &str) -> Result<T, HatchetError>
    where
        T: serde::de::DeserializeOwned;
}

#[derive(Clone, Debug)]
pub struct HatchetClient<A, W, D, E> {
    config: HatchetConfig,
    pub(crate) workflow_client: W,
    pub(crate) dispatcher_client: D,
    pub(crate) event_client: E,
    pub(crate) admin_client: A,
}

impl<A, W, D, E> HatchetClient<A, W, D, E>
where
    A: AdminClientTrait,
    W: WorkflowClientTrait,
    D: DispatcherClientTrait,
    E: EventClientTrait,
{
    pub async fn new(
        config: HatchetConfig,
        admin_client: A,
        workflow_client: W,
        dispatcher_client: D,
        event_client: E,
    ) -> Result<Self, HatchetError> {
        Ok(Self {
            config,
            workflow_client,
            dispatcher_client,
            event_client,
            admin_client,
        })
    }

    async fn create_channel(
        grpc_address: &str,
        tls_strategy: &TlsStrategy,
    ) -> Result<Channel, HatchetError> {
        match tls_strategy {
            TlsStrategy::None => Self::create_insecure_channel(grpc_address).await,
            TlsStrategy::Tls => Self::create_secure_channel(grpc_address).await,
        }
    }

    async fn create_insecure_channel(grpc_address: &str) -> Result<Channel, HatchetError> {
        let channel = Channel::from_shared(format!("http://{}", grpc_address))
            .map_err(|e| HatchetError::InvalidUri(e.to_string()))?
            .connect()
            .await?;

        Ok(channel)
    }

    async fn create_secure_channel(grpc_address: &str) -> Result<Channel, HatchetError> {
        let domain_name = grpc_address
            .split(':')
            .next()
            .ok_or(HatchetError::InvalidGrpcAddress(grpc_address.to_string()))?;

        let tls = ClientTlsConfig::new()
            .domain_name(domain_name)
            .with_native_roots();

        let channel = Channel::from_shared(format!("https://{}", grpc_address))
            .map_err(|e| HatchetError::InvalidUri(e.to_string()))?
            .tls_config(tls)?
            .connect()
            .await?;

        Ok(channel)
    }
}

// Implement from_env with concrete types
impl HatchetClient<AdminClient, WorkflowClient, DispatcherClient, EventClient> {
    pub async fn from_env() -> Result<Self, HatchetError> {
        let config = HatchetConfig::from_env()?;
        let channel = Self::create_channel(&config.grpc_address, &config.tls_strategy).await?;

        let admin_client = AdminClient::new(channel.clone(), config.api_token.clone());
        let workflow_client = WorkflowClient::new(channel.clone(), config.api_token.clone());
        let dispatcher_client = DispatcherClient::new(channel.clone(), config.api_token.clone());
        let event_client = EventClient::new(channel.clone(), config.api_token.clone());
        Self::new(
            config,
            admin_client,
            workflow_client,
            dispatcher_client,
            event_client,
        )
        .await
    }
}

// Concrete implementation of trait
#[async_trait::async_trait]
impl HatchetClientTrait
    for HatchetClient<AdminClient, WorkflowClient, DispatcherClient, EventClient>
{
    async fn get_workflow_run(
        &self,
        run_id: &str,
    ) -> Result<crate::rest::models::GetWorkflowRunResponse, HatchetError> {
        self.api_get(&format!("/api/v1/stable/workflow-runs/{}", run_id))
            .await
    }

    async fn put_workflow(
        &mut self,
        workflow: CreateWorkflowVersionRequest,
    ) -> Result<(), HatchetError> {
        self.admin_client.put_workflow(workflow).await?;
        Ok(())
    }

    async fn trigger_workflow(
        &mut self,
        trigger_workflow_request: TriggerWorkflowRequest,
    ) -> Result<TriggerWorkflowResponse, HatchetError> {
        self.workflow_client
            .trigger_workflow(trigger_workflow_request)
            .await
    }

    async fn put_log(
        &mut self,
        step_run_id: &str,
        message: String,
    ) -> Result<(), crate::HatchetError> {
        self.event_client.put_log(step_run_id, message).await
    }

    async fn send_step_action_event(&mut self, event: StepActionEvent) -> Result<(), HatchetError> {
        self.dispatcher_client.send_step_action_event(event).await
    }

    async fn register_worker(
        &mut self,
        registration: WorkerRegisterRequest,
    ) -> Result<WorkerRegisterResponse, HatchetError> {
        self.dispatcher_client.register_worker(registration).await
    }

    async fn heartbeat(&mut self, worker_id: &str) -> Result<(), HatchetError> {
        self.dispatcher_client.heartbeat(worker_id).await
    }

    async fn listen(
        &mut self,
        worker_id: &str,
    ) -> Result<tonic::Streaming<AssignedAction>, HatchetError> {
        self.dispatcher_client.listen(worker_id).await
    }

    async fn api_get<T>(&self, path: &str) -> Result<T, HatchetError>
    where
        T: serde::de::DeserializeOwned,
    {
        let api_client = crate::rest::ApiClient::new(
            self.config.server_url.clone(),
            self.config.api_token.clone(),
        );

        api_client.get::<T>(path).await
    }
}
