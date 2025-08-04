use std::sync::Arc;

use crate::HatchetClient;
use crate::grpc::events;
use crate::grpc::events::PutLogRequest;
use crate::utils::proto_timestamp_now;

pub(crate) struct EventClient {
    client: Arc<HatchetClient>,
}

impl EventClient {
    pub(crate) fn new(client: Arc<HatchetClient>) -> Self {
        Self { client: client }
    }
    pub(crate) async fn put_log(
        &self,
        step_run_id: &str,
        message: String,
    ) -> Result<(), crate::HatchetError> {
        let request = tonic::Request::new(PutLogRequest {
            step_run_id: step_run_id.to_string(),
            created_at: Some(proto_timestamp_now()?),
            message: message,
            level: None,
            metadata: String::from(""),
        });

        self.client
            .grpc_unary(request, |channel, request| async move {
                let mut client = events::events_service_client::EventsServiceClient::new(channel);
                client.put_log(request).await
            })
            .await?;
        Ok(())
    }
}
