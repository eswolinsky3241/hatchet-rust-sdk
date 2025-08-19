use std::fmt::Debug;

use crate::grpc::v0::events::{PutLogRequest, events_service_client};
use crate::utils::proto_timestamp_now;

#[async_trait::async_trait]
pub trait EventClientTrait: Clone + Debug + Send + Sync {
    async fn put_log(
        &mut self,
        step_run_id: &str,
        message: String,
    ) -> Result<(), crate::HatchetError>;
}

#[derive(Debug, Clone)]
pub(crate) struct EventClient {
    client: events_service_client::EventsServiceClient<tonic::transport::Channel>,
    api_token: String,
}

impl EventClient {
    pub(crate) fn new(channel: tonic::transport::Channel, api_token: String) -> Self {
        let client = events_service_client::EventsServiceClient::new(channel);
        Self { client, api_token }
    }
}

#[async_trait::async_trait]
impl EventClientTrait for EventClient {
    async fn put_log(
        &mut self,
        step_run_id: &str,
        message: String,
    ) -> Result<(), crate::HatchetError> {
        let mut request = tonic::Request::new(PutLogRequest {
            step_run_id: step_run_id.to_string(),
            created_at: Some(proto_timestamp_now()?),
            message: message,
            level: None,
            metadata: String::from(""),
            task_retry_count: None,
        });

        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;

        self.client.put_log(request).await?;
        Ok(())
    }
}
