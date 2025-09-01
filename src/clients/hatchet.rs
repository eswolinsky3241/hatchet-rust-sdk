use super::grpc::{AdminClient, DispatcherClient, EventClient, WorkflowClient};
use crate::clients::rest::apis::configuration::Configuration;
use crate::config::{HatchetConfig, TlsStrategy};
use crate::error::HatchetError;
use crate::features::runs::RunsClient;
use std::sync::Arc;
use tonic::transport::{Channel, ClientTlsConfig};

/// The main client for interacting with the Hatchet API.
#[derive(Clone, Debug)]
pub struct Hatchet {
    server_url: String,
    api_token: String,
    pub(crate) workflow_client: WorkflowClient,
    pub(crate) dispatcher_client: DispatcherClient,
    pub(crate) event_client: EventClient,
    pub(crate) admin_client: AdminClient,
    pub workflow_rest_client: RunsClient,
}

impl Hatchet {
    async fn new(
        server_url: String,
        api_token: String,
        admin_client: AdminClient,
        workflow_client: WorkflowClient,
        dispatcher_client: DispatcherClient,
        event_client: EventClient,
        workflow_rest_client: RunsClient,
    ) -> Result<Self, HatchetError> {
        Ok(Self {
            server_url,
            api_token,
            workflow_client,
            dispatcher_client,
            event_client,
            admin_client,
            workflow_rest_client,
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
            .await
            .map_err(|e| HatchetError::GrpcConnect(e.to_string()))?;

        Ok(channel)
    }

    async fn create_secure_channel(grpc_address: &str) -> Result<Channel, HatchetError> {
        rustls::crypto::ring::default_provider()
            .install_default()
            .map_err(|_| return HatchetError::CryptoProvider)?;

        let domain_name = grpc_address
            .split(':')
            .next()
            .ok_or(HatchetError::InvalidGrpcAddress(grpc_address.to_string()))?;

        let tls = ClientTlsConfig::new()
            .domain_name(domain_name)
            .with_native_roots();

        let channel = Channel::from_shared(format!("https://{}", grpc_address))
            .map_err(|e| HatchetError::InvalidUri(e.to_string()))?
            .tls_config(tls)
            .map_err(|e| HatchetError::GrpcConnect(e.to_string()))?
            .connect()
            .await
            .map_err(|e| HatchetError::GrpcConnect(e.to_string()))?;

        Ok(channel)
    }

    /// Create a client from environment variables.
    /// Set the HATCHET_CLIENT_TOKEN environment variable to your Hatchet API token.
    /// Set the HATCHET_CLIENT_TLS_STRATEGY environment variable to either "none" or "tls" (defaults to "tls").
    pub async fn from_env() -> Result<Self, HatchetError> {
        let config = HatchetConfig::from_env()?;

        let tls_strategy = match config.tls_strategy {
            TlsStrategy::None => "none",
            TlsStrategy::Tls => "tls",
        };

        Ok(Self::from_token(
            &config.server_url,
            &config.grpc_address,
            &config.api_token,
            &tls_strategy,
        )
        .await?)
    }

    pub async fn from_token(
        server_url: &str,
        grpc_broadcast_address: &str,
        token: &str,
        tls_strategy: &str,
    ) -> Result<Self, HatchetError> {
        let config = HatchetConfig::new(token, tls_strategy)?;
        let channel = Self::create_channel(&grpc_broadcast_address, &config.tls_strategy).await?;

        let admin_client = AdminClient::new(channel.clone(), config.api_token.clone());
        let workflow_client = WorkflowClient::new(channel.clone(), config.api_token.clone());
        let dispatcher_client = DispatcherClient::new(channel.clone(), config.api_token.clone());
        let event_client = EventClient::new(channel.clone(), config.api_token.clone());

        let rest_configuration = Arc::new(Configuration {
            base_path: server_url.to_string(),
            user_agent: None,
            client: reqwest::Client::new(),
            basic_auth: None,
            oauth_access_token: None,
            bearer_access_token: Some(config.api_token.clone()),
            api_key: None,
        });
        let workflow_rest_client = RunsClient::new(rest_configuration.clone());

        Self::new(
            server_url.to_string(),
            config.api_token,
            admin_client,
            workflow_client,
            dispatcher_client,
            event_client,
            workflow_rest_client,
        )
        .await
    }

    /// Create a new workflow.
    ///
    /// ```no_run
    /// use hatchet_sdk::{Context, Hatchet, EmptyModel};
    /// #[tokio::main]
    /// async fn main() {
    ///     let hatchet = Hatchet::from_env().await.unwrap();
    ///     let workflow = hatchet.workflow::<EmptyModel, EmptyModel>()
    ///         .name(String::from("my-workflow"))
    ///         .build()
    ///         .add_task(hatchet.task("my-task", async move |input: EmptyModel, _ctx: Context| -> anyhow::Result<EmptyModel> {
    ///             Ok(EmptyModel)
    ///         }))
    ///         .unwrap();
    /// }
    /// ```
    pub fn workflow<I, O>(&self) -> crate::runnables::WorkflowBuilder<I, O>
    where
        I: serde::Serialize + Send + Sync,
        O: serde::de::DeserializeOwned + Send + Sync,
    {
        crate::runnables::WorkflowBuilder::<I, O>::default().client(self.clone())
    }

    /// Create a new task.
    ///
    /// ```no_run
    /// use hatchet_sdk::{Context, Hatchet, EmptyModel};
    /// #[tokio::main]
    /// async fn main() {
    ///     let hatchet = Hatchet::from_env().await.unwrap();
    ///     let task = hatchet.task("my-task", async move |input: EmptyModel, _ctx: Context| -> anyhow::Result<EmptyModel> {
    ///         Ok(EmptyModel)
    ///     });
    /// }
    ///
    /// ```
    pub fn task<I, O, E, F, Fut>(
        &self,
        name: &str,
        handler: F,
    ) -> crate::runnables::task::TaskBuilder<I, O, E>
    where
        I: serde::Serialize + serde::de::DeserializeOwned + Send + Sync + 'static,
        O: serde::Serialize + Send + Sync + 'static,
        E: Into<Box<dyn std::error::Error + Send + Sync>> + Send + 'static,
        F: FnOnce(I, crate::context::Context) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<O, E>> + Send + 'static,
    {
        let handler = Arc::new(move |input: I, ctx: crate::context::Context| {
            let handler_clone = handler.clone();
            Box::pin(handler_clone(input, ctx))
                as std::pin::Pin<Box<dyn Future<Output = Result<O, E>> + Send>>
        });
        crate::runnables::task::TaskBuilder::<I, O, E>::default()
            .name(name.to_string())
            .handler(handler)
            .client(self.clone())
    }

    /// Create a new worker.
    ///
    /// ```no_run
    /// use hatchet_sdk::{Hatchet};
    /// #[tokio::main]
    /// async fn main() {
    ///     let hatchet = Hatchet::from_env().await.unwrap();
    ///     let worker = hatchet.worker().name(String::from("my-worker")).max_runs(5).build();
    /// }
    /// ```
    pub fn worker(&self) -> crate::worker::worker::WorkerBuilder {
        crate::worker::worker::WorkerBuilder::default().client(self.clone())
    }
}
