use super::grpc::{AdminClient, DispatcherClient, EventClient, WorkflowClient};
use crate::Configuration;
use crate::HatchetError;
use crate::RunsClient;
use crate::{HatchetConfig, TlsStrategy};
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
        let _ = rustls::crypto::ring::default_provider().install_default();
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

    /// Create a new workflow. A workflow is a collection of tasks that can be executed by a worker, often forming a directed acyclic graph (DAG).
    ///
    /// ### Required parameters:
    /// * `name` - The name of the workflow.
    ///
    /// ### Optional builder parameters:
    /// * `description` - An optional description for the workflow.
    /// * `version` - A version for the workflow.
    /// * `default_priority` - The priority of the workflow. Higher values will cause this workflow to have priority in scheduling.
    /// * `on_events` - A list of event triggers for the workflow - events which cause the workflow to be run.
    /// * `cron_triggers` - A list of cron triggers for the workflow.
    ///
    /// ### Examples:
    /// ```no_run
    /// use hatchet_sdk::{Context, Hatchet, EmptyModel};
    /// #[tokio::main]
    /// async fn main() {
    ///     let hatchet = Hatchet::from_env().await.unwrap();
    ///     let workflow = hatchet.workflow::<EmptyModel, EmptyModel>("my-workflow")
    ///         .build()
    ///         .unwrap()
    ///         .add_task(&hatchet.task("my-task", async move |input: EmptyModel, _ctx: Context| -> anyhow::Result<EmptyModel> {
    ///             Ok(EmptyModel)
    ///         }).build().unwrap());
    /// }
    /// ```
    pub fn workflow<I, O>(&self, name: &str) -> crate::runnables::WorkflowBuilder<I, O>
    where
        I: serde::Serialize + Send + Sync,
        O: serde::de::DeserializeOwned + Send + Sync,
    {
        crate::runnables::WorkflowBuilder::<I, O>::default()
            .name(name.to_string())
            .client(self.clone())
    }

    /// Returns a builder for a new task. A task is a unit of work that can be executed by a worker.
    /// Provide a task name and a handler function that will be executed by the worker. The handler
    /// is an async function or closure that accepts two arguments, the task input and a task context.
    /// The handler should return an `anyhow::Result` containing the task output.
    /// The input and output types should be serializable and deserializable.
    ///
    /// ### Required parameters:
    /// * `name` - The name of the task.
    /// * `handler` - The function handler.
    ///
    /// ### Optional builder parameters:
    /// * `description` - An optional description for the task.
    /// * `version` - A version for the task.
    /// * `on_events` - A list of event triggers for the task - events which cause the task to be run.
    /// * `cron_triggers` - A list of cron triggers for the task.
    /// * `default_priority` - The priority of the task. Higher values will cause this task to have priority in scheduling.
    /// * `default_filters` - A list of filters to create when the task is created.
    /// * `retries` - The number of times to retry the task before failing.
    /// * `schedule_timeout` - The maximum time allowed for scheduling the task. Defaults to five minutes.
    /// * `execution_timeout` - The maximum time allowed for executing the task. Defaults to one minute.
    ///
    ///
    /// ### Examples:
    /// ```no_run
    /// use hatchet_sdk::{Context, Hatchet, EmptyModel};
    /// #[tokio::main]
    /// async fn main() {
    ///     let hatchet = Hatchet::from_env().await.unwrap();
    ///     let task = hatchet.task("my-task", async move |input: EmptyModel, _ctx: Context| -> anyhow::Result<EmptyModel> {
    ///         Ok(EmptyModel)
    ///     })
    ///     .description(String::from("My task description"))
    ///     .version(String::from("1.0.0"))
    ///     .on_events(vec![String::from("my-event:trigger")])
    ///     .cron_triggers(vec![String::from("0 0 * * *")])
    ///     .default_priority(10)
    ///     .schedule_timeout(std::time::Duration::from_secs(300))
    ///     .execution_timeout(std::time::Duration::from_secs(60))
    ///     .retries(3)
    ///     .build()
    ///     .unwrap();
    /// }
    ///
    /// ```
    pub fn task<I, O, F, Fut>(&self, name: &str, handler: F) -> crate::runnables::TaskBuilder<I, O>
    where
        I: serde::Serialize + serde::de::DeserializeOwned + Send + Sync + 'static,
        O: serde::Serialize + Send + Sync + 'static,
        F: FnOnce(I, crate::context::Context) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = anyhow::Result<O>> + Send + 'static,
    {
        let handler = Arc::new(move |input: I, ctx: crate::context::Context| {
            let handler_clone = handler.clone();
            Box::pin(handler_clone(input, ctx))
                as std::pin::Pin<Box<dyn Future<Output = anyhow::Result<O>> + Send>>
        });
        crate::runnables::TaskBuilder::<I, O>::default()
            .name(name.to_string())
            .handler(handler)
            .client(self.clone())
    }

    /// Returns a builder for a new worker. A worker is a container for tasks that can be executed by a worker.
    /// Provide a worker name and a number of slots. The slots parameter is the number of tasks that can be executed by the worker concurrently.
    /// The worker will be registered with Hatchet and will start executing tasks when started.
    ///
    /// ### Required parameters:
    /// * `name` - The name of the worker.
    /// * `slots` - The number of slots for the worker.
    ///
    /// ### Optional builder parameters:
    /// * `labels` - A map of labels for the worker.
    ///
    /// ### Examples:
    /// ```no_run
    /// use hatchet_sdk::{Hatchet};
    /// #[tokio::main]
    /// async fn main() {
    ///     let hatchet = Hatchet::from_env().await.unwrap();
    ///     let worker = hatchet.worker("my-worker")
    ///     .slots(5)
    ///     .labels(std::collections::HashMap::from([(String::from("env"), String::from("dev"))]))
    ///     .build()
    ///     .unwrap();
    /// }
    /// ```
    pub fn worker(&self, name: &str) -> crate::worker::worker::WorkerBuilder {
        crate::worker::worker::WorkerBuilder::default()
            .name(name.to_string())
            .client(self.clone())
    }
}
