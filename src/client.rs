use tonic::transport::{Channel, ClientTlsConfig};

use crate::clients::{
    AdminClient, AdminClientTrait, DispatcherClient, DispatcherClientTrait, EventClient,
    EventClientTrait, WorkflowClient, WorkflowClientTrait,
};
use crate::config::{HatchetConfig, TlsStrategy};
use crate::error::HatchetError;
use crate::grpc::v0::dispatcher::{
    AssignedAction, StepActionEvent, WorkerRegisterRequest, WorkerRegisterResponse,
};
use crate::grpc::v0::workflows::{TriggerWorkflowRequest, TriggerWorkflowResponse};
use crate::grpc::v1::workflows::{CreateTaskOpts, CreateWorkflowVersionRequest};

#[async_trait::async_trait]
pub(crate) trait HatchetClientTrait: Clone + Send + Sync + 'static {
    async fn get_workflow_run(
        &self,
        run_id: &str,
    ) -> Result<crate::rest::models::GetWorkflowRunResponse, HatchetError>;

    async fn put_workflow(
        &mut self,
        name: &str,
        tasks: Vec<CreateTaskOpts>,
        event_triggers: Option<Vec<String>>,
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
    server_url: String,
    api_token: String,
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
        server_url: String,
        api_token: String,
        admin_client: A,
        workflow_client: W,
        dispatcher_client: D,
        event_client: E,
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

impl HatchetClient<AdminClient, WorkflowClient, DispatcherClient, EventClient> {
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
            admin_client,
            workflow_client,
            dispatcher_client,
            event_client,
        )
        .await
    }
}

#[async_trait::async_trait]
impl<A, W, D, E> HatchetClientTrait for HatchetClient<A, W, D, E>
where
    A: AdminClientTrait,
    W: WorkflowClientTrait,
    D: DispatcherClientTrait,
    E: EventClientTrait,
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
        name: &str,
        tasks: Vec<CreateTaskOpts>,
        event_triggers: Option<Vec<String>>,
    ) -> Result<(), HatchetError> {
        let workflow = CreateWorkflowVersionRequest {
            name: name.to_string(),
            tasks,
            event_triggers: event_triggers.unwrap_or(vec![]),
            cron_triggers: vec![],
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
        let api_client =
            crate::rest::ApiClient::new(self.server_url.clone(), self.api_token.clone());

        api_client.get::<T>(path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::predicate::*;
    use mockall::*;

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

    #[tokio::test]
    async fn test_put_workflow() {
        let mut admin_client = MockAdminClient::new();
        let workflow_client = MockWorkflowClient::new();
        let dispatcher_client = MockDispatcherClient::new();
        let event_client = MockEventClient::new();

        admin_client
            .expect_put_workflow()
            .with(eq(CreateWorkflowVersionRequest {
                name: "test-workflow".to_string(),
                description: "".to_string(),
                version: "".to_string(),
                event_triggers: vec!["test-event".to_string()],
                cron_triggers: vec![],
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

        let mut client = HatchetClient::new(
            String::from("https://hatchet.com"),
            String::from("part0.part1.part2"),
            admin_client,
            workflow_client,
            dispatcher_client,
            event_client,
        )
        .await
        .unwrap();

        let workflow_run = client
            .put_workflow(
                "test-workflow",
                vec![],
                Some(vec!["test-event".to_string()]),
            )
            .await
            .unwrap();

        assert_eq!(workflow_run, ());
    }
}
