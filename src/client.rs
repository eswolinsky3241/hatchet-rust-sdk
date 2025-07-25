use tonic::metadata::MetadataValue;
use tonic::transport::{Channel, ClientTlsConfig};
use tonic::{Request, Response};

use crate::config::HatchetConfig;
use crate::error::HatchetError;

pub mod workflows {
    tonic::include_proto!("_");
}
pub mod dispatcher {
    tonic::include_proto!("_");
}

#[derive(Debug)]
pub struct HatchetClient {
    config: HatchetConfig,
}

impl HatchetClient {
    pub async fn new(config: HatchetConfig) -> Result<Self, HatchetError> {
        Ok(Self { config })
    }

    pub fn from_env() -> Result<Self, HatchetError> {
        Ok(Self {
            config: HatchetConfig::from_env()?,
        })
    }

    pub async fn grpc_stream<Req, Resp, F, Fut>(
        &self,
        mut request: Request<Req>,
        service_call: F,
    ) -> Result<Response<Resp>, HatchetError>
    where
        F: FnOnce(Channel, Request<Req>) -> Fut,
        Fut: std::future::Future<Output = Result<Response<Resp>, tonic::Status>>,
    {
        self.add_auth_header(&mut request)?;
        let channel = self.create_channel().await?;
        let stream = service_call(channel, request)
            .await
            .map_err(HatchetError::GrpcCall)?;

        Ok(stream)
    }

    pub async fn grpc_unary<Req, Resp, F, Fut>(
        &self,
        mut request: Request<Req>,
        service_call: F,
    ) -> Result<Response<Resp>, HatchetError>
    where
        F: FnOnce(Channel, Request<Req>) -> Fut,
        Fut: std::future::Future<Output = Result<Response<Resp>, tonic::Status>>,
    {
        self.add_auth_header(&mut request)?;
        let channel = self.create_channel().await?;
        service_call(channel, request)
            .await
            .map_err(HatchetError::GrpcCall)
    }

    pub async fn api_get<T>(&self, path: &str) -> Result<T, HatchetError>
    where
        T: serde::de::DeserializeOwned,
    {
        let api_client = crate::rest::ApiClient::new(
            self.config.server_url.clone(),
            self.config.api_token.clone(),
        );

        api_client.get::<T>(path).await
    }

    async fn create_channel(&self) -> Result<Channel, HatchetError> {
        let tls_strategy =
            std::env::var("HATCHET_CLIENT_TLS_STRATEGY").unwrap_or("tls".to_string());

        let domain_name = self
            .config
            .grpc_address
            .split(':')
            .next()
            .ok_or(HatchetError::MissingTokenField("grpc_broadcast_address"))?;

        if tls_strategy.to_lowercase() == "none" {
            self.create_insecure_channel().await
        } else {
            self.create_secure_channel(domain_name).await
        }
    }

    async fn create_insecure_channel(&self) -> Result<Channel, HatchetError> {
        Channel::from_shared(format!("http://{}", self.config.grpc_address))
            .map_err(|e| HatchetError::InvalidUri { uri: e.to_string() })?
            .connect()
            .await
            .map_err(HatchetError::GrpcConnect)
    }

    async fn create_secure_channel(&self, domain_name: &str) -> Result<Channel, HatchetError> {
        let tls = ClientTlsConfig::new()
            .domain_name(domain_name)
            .with_native_roots();

        Channel::from_shared(format!("https://{}", self.config.grpc_address))
            .map_err(|e| HatchetError::InvalidUri { uri: e.to_string() })?
            .tls_config(tls)?
            .connect()
            .await
            .map_err(HatchetError::GrpcConnect)
    }

    fn add_auth_header<T>(&self, request: &mut Request<T>) -> Result<(), HatchetError> {
        let token_header: MetadataValue<_> = format!("Bearer {}", self.config.api_token)
            .parse()
            .map_err(HatchetError::InvalidAuthHeader)?;

        request.metadata_mut().insert("authorization", token_header);
        Ok(())
    }
}
