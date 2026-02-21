use super::v0::events::{PutLogRequest, PutStreamEventRequest, events_service_client};
use crate::HatchetError;
use crate::proto_timestamp_now;

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

impl EventClient {
    pub async fn put_log(
        &mut self,
        task_run_external_id: &str,
        message: String,
    ) -> Result<(), crate::HatchetError> {
        let mut request = tonic::Request::new(PutLogRequest {
            task_run_external_id: task_run_external_id.to_string(),
            created_at: Some(proto_timestamp_now()?),
            message,
            level: None,
            metadata: String::from(""),
            task_retry_count: None,
        });

        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;

        self.client
            .put_log(request)
            .await
            .map_err(|e| HatchetError::GrpcErrorStatus(e.message().to_string()))?;
        Ok(())
    }

    pub async fn put_stream_event(
        &mut self,
        task_run_external_id: &str,
        message: Vec<u8>,
        event_index: Option<i64>,
    ) -> Result<(), HatchetError> {
        let mut request = tonic::Request::new(PutStreamEventRequest {
            task_run_external_id: task_run_external_id.to_string(),
            created_at: Some(proto_timestamp_now()?),
            message,
            metadata: String::new(),
            event_index,
        });

        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;

        self.client
            .put_stream_event(request)
            .await
            .map_err(|e| HatchetError::GrpcErrorStatus(e.message().to_string()))?;
        Ok(())
    }
}
