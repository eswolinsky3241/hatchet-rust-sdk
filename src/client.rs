use tonic::metadata::MetadataValue;
use tonic::transport::{Channel, ClientTlsConfig};
use tonic::{Request, Response};

use crate::config::{HatchetConfig, TlsStrategy};
use crate::error::HatchetError;
use crate::grpc::v0::workflows::{
    TriggerWorkflowRequest,
    TriggerWorkflowResponse,
    workflow_service_client,
};

#[cfg_attr(test, mockall::automock)]
#[async_trait::async_trait]
pub(crate) trait HatchetClientTrait: Send + Sync {
    async fn trigger_workflow(
        &self,
        request: TriggerWorkflowRequest,
    ) -> Result<TriggerWorkflowResponse, HatchetError>;

    async fn get_workflow_run(
        &self,
        run_id: &str,
    ) -> Result<crate::rest::models::GetWorkflowRunResponse, HatchetError>;
}

pub mod workflows {
    tonic::include_proto!("_");
}
pub mod dispatcher {
    tonic::include_proto!("_");
}

#[derive(Clone, Debug)]
pub struct HatchetClient {
    config: HatchetConfig,
}

impl HatchetClient {
    pub fn new(config: HatchetConfig) -> Result<Self, HatchetError> {
        Ok(Self { config })
    }

    pub fn from_env() -> Result<Self, HatchetError> {
        Ok(Self {
            config: HatchetConfig::from_env()?,
        })
    }

    pub(crate) async fn grpc_stream<Req, Resp, F, Fut>(
        &self,
        mut request: Request<Req>,
        service_call: F,
    ) -> Result<Response<Resp>, HatchetError>
    where
        F: FnOnce(Channel, Request<Req>) -> Fut,
        Fut: std::future::Future<Output = Result<Response<Resp>, tonic::Status>>,
    {
        self.add_auth_header(&mut request)?;
        let channel = self.create_channel(&self.config.tls_strategy).await?;
        let stream = service_call(channel, request)
            .await
            .map_err(HatchetError::GrpcCall)?;

        Ok(stream)
    }

    pub(crate) async fn grpc_unary<Req, Resp, F, Fut>(
        &self,
        mut request: Request<Req>,
        service_call: F,
    ) -> Result<Response<Resp>, HatchetError>
    where
        F: FnOnce(Channel, Request<Req>) -> Fut,
        Fut: std::future::Future<Output = Result<Response<Resp>, tonic::Status>>,
    {
        self.add_auth_header(&mut request)?;
        let channel = self.create_channel(&self.config.tls_strategy).await?;
        service_call(channel, request)
            .await
            .map_err(HatchetError::GrpcCall)
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

    async fn create_channel(&self, tls_strategy: &TlsStrategy) -> Result<Channel, HatchetError> {
        let domain_name =
            self.config
                .grpc_address
                .split(':')
                .next()
                .ok_or(HatchetError::InvalidGrpcAddress(
                    self.config.grpc_address.clone(),
                ))?;

        match tls_strategy {
            TlsStrategy::None => self.create_insecure_channel().await,
            TlsStrategy::Tls => self.create_secure_channel(domain_name).await,
        }
    }

    async fn create_insecure_channel(&self) -> Result<Channel, HatchetError> {
        let channel = Channel::from_shared(format!("http://{}", self.config.grpc_address))
            .map_err(|e| HatchetError::InvalidUri(e.to_string()))?
            .connect()
            .await?;

        Ok(channel)
    }

    async fn create_secure_channel(&self, domain_name: &str) -> Result<Channel, HatchetError> {
        let tls = ClientTlsConfig::new()
            .domain_name(domain_name)
            .with_native_roots();

        let channel = Channel::from_shared(format!("https://{}", self.config.grpc_address))
            .map_err(|e| HatchetError::InvalidUri(e.to_string()))?
            .tls_config(tls)?
            .connect()
            .await?;

        Ok(channel)
    }

    fn add_auth_header<T>(&self, request: &mut Request<T>) -> Result<(), HatchetError> {
        let token_header: MetadataValue<_> = format!("Bearer {}", self.config.api_token).parse()?;

        request.metadata_mut().insert("authorization", token_header);
        Ok(())
    }
}

#[async_trait::async_trait]
impl HatchetClientTrait for HatchetClient {
    async fn trigger_workflow(
        &self,
        request: TriggerWorkflowRequest,
    ) -> Result<TriggerWorkflowResponse, HatchetError> {
        use workflow_service_client::WorkflowServiceClient;

        let response = self
            .grpc_unary(Request::new(request), |channel, request| async move {
                let mut client = WorkflowServiceClient::new(channel);
                client.trigger_workflow(request).await
            })
            .await?;

        Ok(response.into_inner())
    }

    async fn get_workflow_run(
        &self,
        run_id: &str,
    ) -> Result<crate::rest::models::GetWorkflowRunResponse, HatchetError> {
        self.api_get(&format!("/api/v1/stable/workflow-runs/{}", run_id))
            .await
    }
}

#[async_trait::async_trait]
impl<T> HatchetClientTrait for std::sync::Arc<T>
where
    T: HatchetClientTrait + ?Sized,
{
    async fn trigger_workflow(
        &self,
        request: crate::grpc::v0::workflows::TriggerWorkflowRequest,
    ) -> Result<crate::grpc::v0::workflows::TriggerWorkflowResponse, HatchetError> {
        (**self).trigger_workflow(request).await
    }

    async fn get_workflow_run(
        &self,
        run_id: &str,
    ) -> Result<crate::rest::models::GetWorkflowRunResponse, HatchetError> {
        (**self).get_workflow_run(run_id).await
    }
}
