use dyn_clone::DynClone;
use serde_json::Value;
use tonic::transport::{Channel, ClientTlsConfig};

use crate::config::{HatchetConfig, TlsStrategy};
use crate::error::HatchetError;
use crate::grpc::v0::dispatcher::{
    AssignedAction, StepActionEvent, WorkerRegisterRequest, WorkerRegisterResponse,
};
use crate::grpc::v0::workflows::{TriggerWorkflowRequest, TriggerWorkflowResponse};
use crate::grpc::v1::workflows::{CreateTaskOpts, CreateWorkflowVersionRequest};
use crate::grpc::{
    AdminClient, AdminClientTrait, DispatcherClient, DispatcherClientTrait, EventClient,
    EventClientTrait, WorkflowClient, WorkflowClientTrait,
};
use crate::rest::models::GetWorkflowRunResponse;
use crate::workflows::DefaultFilter;

#[async_trait::async_trait]
pub trait HatchetClientTrait: std::fmt::Debug + Send + Sync + DynClone + 'static {
    async fn get_workflow_run(
        &self,
        run_id: &str,
    ) -> Result<crate::rest::models::GetWorkflowRunResponse, HatchetError>;

    async fn put_workflow(
        &mut self,
        name: &str,
        tasks: Vec<CreateTaskOpts>,
        event_triggers: Option<Vec<String>>,
        cron_triggers: Vec<String>,
    ) -> Result<(), HatchetError>;

    async fn trigger_workflow(
        &mut self,
        workflow_name: &str,
        input: serde_json::Value,
        additional_metadata: Option<Value>,
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

    async fn api_get(&self, path: &str) -> Result<GetWorkflowRunResponse, HatchetError>;
}

dyn_clone::clone_trait_object!(HatchetClientTrait);

#[derive(Clone, Debug)]
pub struct HatchetClient {
    server_url: String,
    api_token: String,
    workflow_client: Box<dyn WorkflowClientTrait + Send + Sync>,
    dispatcher_client: Box<dyn DispatcherClientTrait + Send + Sync>,
    event_client: Box<dyn EventClientTrait + Send + Sync>,
    admin_client: Box<dyn AdminClientTrait + Send + Sync>,
}

impl HatchetClient {
    pub async fn new(
        server_url: String,
        api_token: String,
        admin_client: Box<dyn AdminClientTrait + Send + Sync>,
        workflow_client: Box<dyn WorkflowClientTrait + Send + Sync>,
        dispatcher_client: Box<dyn DispatcherClientTrait + Send + Sync>,
        event_client: Box<dyn EventClientTrait + Send + Sync>,
    ) -> Result<Self, HatchetError> {
        Ok(Self {
            server_url,
            api_token,
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
            .tls_config(tls)?
            .connect()
            .await?;

        Ok(channel)
    }

    pub async fn from_env() -> Result<Self, HatchetError> {
        let config = HatchetConfig::from_env()?;
        let channel = Self::create_channel(&config.grpc_address, &config.tls_strategy).await?;

        let admin_client = AdminClient::new(channel.clone(), config.api_token.clone());
        let workflow_client = WorkflowClient::new(channel.clone(), config.api_token.clone());
        let dispatcher_client = DispatcherClient::new(channel.clone(), config.api_token.clone());
        let event_client = EventClient::new(channel.clone(), config.api_token.clone());
        Self::new(
            config.server_url,
            config.api_token,
            Box::new(admin_client),
            Box::new(workflow_client),
            Box::new(dispatcher_client),
            Box::new(event_client),
        )
        .await
    }

    pub async fn from_token(token: &str, tls_strategy: &str) -> Result<Self, HatchetError> {
        println!("{}", token);
        let config = HatchetConfig::new(token, tls_strategy)?;
        let channel = Self::create_channel(&config.grpc_address, &config.tls_strategy).await?;

        let admin_client = AdminClient::new(channel.clone(), config.api_token.clone());
        let workflow_client = WorkflowClient::new(channel.clone(), config.api_token.clone());
        let dispatcher_client = DispatcherClient::new(channel.clone(), config.api_token.clone());
        let event_client = EventClient::new(channel.clone(), config.api_token.clone());
        Self::new(
            config.server_url,
            config.api_token,
            Box::new(admin_client),
            Box::new(workflow_client),
            Box::new(dispatcher_client),
            Box::new(event_client),
        )
        .await
    }

    pub fn new_workflow<I, O>(
        &self,
        workflow_name: &str,
        on_events: Vec<String>,
        cron_triggers: Vec<String>,
        default_filters: Vec<DefaultFilter>,
    ) -> crate::workflows::Workflow<I, O>
    where
        I: serde::Serialize + Send + Sync,
        O: serde::de::DeserializeOwned + Send + Sync,
    {
        crate::workflows::Workflow::<I, O>::new(
            workflow_name,
            Box::new(self.clone()),
            on_events,
            cron_triggers,
            default_filters,
        )
    }

    pub fn new_task<I, O, E, F, Fut>(&self, name: &str, f: F) -> crate::workflows::Task<I, O, E>
    where
        I: serde::de::DeserializeOwned + Send + Sync + 'static,
        O: serde::Serialize + Send + Sync + 'static,
        E: Into<Box<dyn std::error::Error + Send + Sync>> + Send + 'static,
        F: FnOnce(I, crate::workflows::context::Context) -> Fut + Send + Sync + Clone + 'static,
        Fut: std::future::Future<Output = Result<O, E>> + Send + 'static,
    {
        crate::workflows::Task::<I, O, E>::new(name, f)
    }
}

#[async_trait::async_trait]
impl HatchetClientTrait for HatchetClient {
    async fn get_workflow_run(
        &self,
        run_id: &str,
    ) -> Result<crate::rest::models::GetWorkflowRunResponse, HatchetError> {
        self.api_get(&format!("/api/v1/stable/workflow-runs/{}", run_id))
            .await
    }

    async fn put_workflow(
        &mut self,
        name: &str,
        tasks: Vec<CreateTaskOpts>,
        event_triggers: Option<Vec<String>>,
        cron_triggers: Vec<String>,
    ) -> Result<(), HatchetError> {
        let workflow = CreateWorkflowVersionRequest {
            name: name.to_string(),
            tasks,
            event_triggers: event_triggers.unwrap_or(vec![]),
            cron_triggers: cron_triggers,
            description: String::from(""),
            version: String::from(""),
            concurrency: None,
            cron_input: None,
            on_failure_task: None,
            sticky: None,
            default_priority: None,
            concurrency_arr: vec![],
            default_filters: vec![],
        };
        self.admin_client.put_workflow(workflow).await?;
        Ok(())
    }

    async fn trigger_workflow(
        &mut self,
        workflow_name: &str,
        input: serde_json::Value,
        additional_metadata: Option<Value>,
    ) -> Result<TriggerWorkflowResponse, HatchetError> {
        let request = TriggerWorkflowRequest {
            name: workflow_name.to_string(),
            input: input.to_string(),
            parent_id: None,
            parent_step_run_id: None,
            child_index: None,
            child_key: None,
            additional_metadata: additional_metadata.map(|v| v.to_string()),
            desired_worker_id: None,
            priority: None,
        };
        self.workflow_client.trigger_workflow(request).await
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

    async fn api_get(&self, path: &str) -> Result<GetWorkflowRunResponse, HatchetError> {
        let api_client =
            crate::rest::ApiClient::new(self.server_url.clone(), self.api_token.clone());

        api_client.get(path).await
    }
}

#[cfg(test)]
mod tests {
    use mockall::predicate::*;
    use mockall::*;
    use rstest::*;
    use serde_json::json;

    use super::*;

    mock! {
        #[derive(Clone, Debug)]
        AdminClient { }
        #[async_trait::async_trait]
        impl AdminClientTrait for AdminClient {
            async fn put_workflow(&mut self, workflow: CreateWorkflowVersionRequest) -> Result<(), HatchetError>;
        }
        impl Clone for AdminClient {
            fn clone(&self) -> Self;
        }
    }
    mock! {
        #[derive(Debug)]
        EventClient { }
        #[async_trait::async_trait]
        impl EventClientTrait for EventClient {
            async fn put_log(&mut self, step_run_id: &str, message: String) -> Result<(), HatchetError>;
        }
        impl Clone for EventClient {
            fn clone(&self) -> Self;
        }
    }
    mock! {
        #[derive(Debug)]
        WorkflowClient { }
        #[async_trait::async_trait]
        impl WorkflowClientTrait for WorkflowClient {
            async fn trigger_workflow(&mut self, trigger_workflow_request: TriggerWorkflowRequest) -> Result<TriggerWorkflowResponse, HatchetError>;
        }
        impl Clone for WorkflowClient {
            fn clone(&self) -> Self;
        }
    }
    mock! {
        #[derive(Debug)]
        DispatcherClient { }
        #[async_trait::async_trait]
        impl DispatcherClientTrait for DispatcherClient {
            async fn send_step_action_event(&mut self, event: StepActionEvent) -> Result<(), HatchetError>;
            async fn register_worker(&mut self, registration: WorkerRegisterRequest) -> Result<WorkerRegisterResponse, HatchetError>;
            async fn heartbeat(&mut self, worker_id: &str) -> Result<(), HatchetError>;
            async fn listen(&mut self, worker_id: &str) -> Result<tonic::Streaming<AssignedAction>, HatchetError>;
        }
        impl Clone for DispatcherClient {
            fn clone(&self) -> Self;
        }
    }

    #[fixture]
    fn mock_clients() -> (
        MockAdminClient,
        MockWorkflowClient,
        MockDispatcherClient,
        MockEventClient,
    ) {
        (
            MockAdminClient::new(),
            MockWorkflowClient::new(),
            MockDispatcherClient::new(),
            MockEventClient::new(),
        )
    }

    async fn create_test_client(
        admin_client: MockAdminClient,
        workflow_client: MockWorkflowClient,
        dispatcher_client: MockDispatcherClient,
        event_client: MockEventClient,
    ) -> HatchetClient {
        HatchetClient::new(
            String::from("https://hatchet.com"),
            String::from("part0.part1.part2"),
            Box::new(admin_client),
            Box::new(workflow_client),
            Box::new(dispatcher_client),
            Box::new(event_client),
        )
        .await
        .unwrap()
    }

    #[rstest]
    #[tokio::test]
    async fn test_trigger_workflow(
        mock_clients: (
            MockAdminClient,
            MockWorkflowClient,
            MockDispatcherClient,
            MockEventClient,
        ),
    ) {
        let (admin_client, mut workflow_client, dispatcher_client, event_client) = mock_clients;

        workflow_client
            .expect_trigger_workflow()
            .with(eq(TriggerWorkflowRequest {
                name: String::from("test-workflow"),
                input: String::from("{\"key\":\"value\"}"),
                parent_id: None,
                parent_step_run_id: None,
                child_index: None,
                child_key: None,
                additional_metadata: Some(String::from("{\"label\":\"value\"}")),
                desired_worker_id: None,
                priority: None,
            }))
            .returning(|_| {
                Ok(TriggerWorkflowResponse {
                    workflow_run_id: String::from("123"),
                })
            });

        let mut client = create_test_client(
            admin_client,
            workflow_client,
            dispatcher_client,
            event_client,
        )
        .await;

        let workflow_run_id = client
            .trigger_workflow(
                "test-workflow",
                json!({"key": "value"}),
                Some(json!({"label": "value"})),
            )
            .await
            .unwrap();

        assert_eq!(workflow_run_id.workflow_run_id, "123")
    }

    #[rstest]
    #[tokio::test]
    async fn test_log(
        mock_clients: (
            MockAdminClient,
            MockWorkflowClient,
            MockDispatcherClient,
            MockEventClient,
        ),
    ) {
        let (admin_client, workflow_client, dispatcher_client, mut event_client) = mock_clients;

        event_client
            .expect_put_log()
            .with(eq("test-step-run"), eq(String::from("test-message")))
            .returning(|_, _| Ok(()));

        let mut client = create_test_client(
            admin_client,
            workflow_client,
            dispatcher_client,
            event_client,
        )
        .await;

        client
            .put_log("test-step-run", String::from("test-message"))
            .await
            .unwrap();
    }

    #[rstest]
    #[tokio::test]
    async fn test_put_workflow(
        mock_clients: (
            MockAdminClient,
            MockWorkflowClient,
            MockDispatcherClient,
            MockEventClient,
        ),
    ) {
        let (mut admin_client, workflow_client, dispatcher_client, event_client) = mock_clients;

        admin_client
            .expect_put_workflow()
            .with(eq(CreateWorkflowVersionRequest {
                name: "test-workflow".to_string(),
                description: "".to_string(),
                version: "".to_string(),
                event_triggers: vec!["test-event".to_string()],
                cron_triggers: vec!["* * * * *".to_string()],
                tasks: vec![],
                concurrency: None,
                cron_input: None,
                on_failure_task: None,
                sticky: None,
                default_priority: None,
                concurrency_arr: vec![],
                default_filters: vec![],
            }))
            .returning(|_| Ok(()));

        let mut client = create_test_client(
            admin_client,
            workflow_client,
            dispatcher_client,
            event_client,
        )
        .await;

        let workflow_run = client
            .put_workflow(
                "test-workflow",
                vec![],
                Some(vec!["test-event".to_string()]),
                vec![String::from("* * * * *")],
            )
            .await
            .unwrap();

        assert_eq!(workflow_run, ());
    }
}
