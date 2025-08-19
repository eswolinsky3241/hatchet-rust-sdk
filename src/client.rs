use tonic::transport::{Channel, ClientTlsConfig};

use crate::clients::{
    AdminClient,
    AdminClientTrait,
    DispatcherClient,
    EventClient,
    WorkflowClient,
    WorkflowClientTrait,
};
use crate::config::{HatchetConfig, TlsStrategy};
use crate::error::HatchetError;
use crate::grpc::v0::workflows::{TriggerWorkflowRequest, TriggerWorkflowResponse};
use crate::grpc::v1::workflows::CreateWorkflowVersionRequest;

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub(crate) trait HatchetClientTrait: Send + Sync {
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
}

pub(crate) type SafeHatchetClient<A, W> = std::sync::Arc<tokio::sync::Mutex<HatchetClient<A, W>>>;

#[derive(Clone, Debug)]
pub struct HatchetClient<A, W> {
    config: HatchetConfig,
    pub(crate) workflow_client: W,
    pub(crate) dispatcher_client: DispatcherClient,
    pub(crate) event_client: EventClient,
    pub(crate) admin_client: A,
}

impl<A, W> HatchetClient<A, W>
where
    A: AdminClientTrait,
    W: WorkflowClientTrait,
{
    pub async fn new(
        config: HatchetConfig,
        admin_client: A,
        workflow_client: W,
    ) -> Result<Self, HatchetError> {
        let channel = Self::create_channel(&config.grpc_address, &config.tls_strategy).await?;

        let dispatcher_client = DispatcherClient::new(channel.clone(), config.api_token.clone());
        let event_client = EventClient::new(channel.clone(), config.api_token.clone());

        Ok(Self {
            config,
            workflow_client,
            dispatcher_client,
            event_client,
            admin_client,
        })
    }

    pub(crate) async fn api_get<T>(&self, path: &str) -> Result<T, HatchetError>
    where
        T: serde::de::DeserializeOwned,
    {
        let api_client = crate::rest::ApiClient::new(
            self.config.server_url.clone(),
            self.config.api_token.clone(),
        );

        api_client.get::<T>(path).await
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
impl HatchetClient<AdminClient, WorkflowClient> {
    pub async fn from_env() -> Result<Self, HatchetError> {
        let config = HatchetConfig::from_env()?;
        let channel = Self::create_channel(&config.grpc_address, &config.tls_strategy).await?;

        let admin_client = AdminClient::new(channel.clone(), config.api_token.clone());
        let workflow_client = WorkflowClient::new(channel.clone(), config.api_token.clone());
        Self::new(config, admin_client, workflow_client).await
    }
}

// Concrete implementation of trait
#[async_trait::async_trait]
impl HatchetClientTrait for HatchetClient<AdminClient, WorkflowClient> {
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
}
