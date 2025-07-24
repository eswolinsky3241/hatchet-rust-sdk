use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use prost_types::Timestamp;

use crate::task::ErasedTask;
use crate::worker::dispatcher::AssignedAction;

// Responsible for managing task threads and reporting the results back to Hatchet
pub struct TaskDispatcher {
    pub registry: Arc<HashMap<String, Arc<dyn ErasedTask>>>,
    pub client: Arc<crate::client::HatchetClient>,
}

impl TaskDispatcher {
    pub async fn dispatch(
        &self,
        worker_id: Arc<String>,
        message: crate::worker::dispatcher::AssignedAction,
    ) -> Result<(), crate::error::HatchetError> {
        match message.action_type().as_str_name() {
            "START_STEP_RUN" => {
                self.handle_start_step_run(worker_id, message).await?;
            }
            "CANCEL_STEP_RUN" => println!("CANCELING"),
            _ => println!("GOT SOMETHING ELSE"),
        }
        Ok(())
    }

    async fn handle_start_step_run(
        &self,
        worker_id: Arc<String>,
        message: crate::worker::dispatcher::AssignedAction,
    ) -> Result<(), crate::error::HatchetError> {
        // Send initial event (type 1)
        self.send_step_action_event_common(&worker_id, &message, 1, "".to_string())
            .await?;

        let step_id = message.step_run_id.clone();
        println!("[{step_id}] Received task. Starting handler...");

        let raw_json: serde_json::Value = serde_json::from_str(&message.action_payload)
            .expect("could not parse action_payload as JSON");
        let input_value = raw_json
            .get("input")
            .cloned()
            .expect("missing `input` field in action_payload");

        let handler = self.registry.get(&message.action_id).unwrap();
        let result = handler.run_from_json(input_value).await?;
        println!("{result}");

        self.send_step_action_event_common(&worker_id, &message, 2, result.to_string())
            .await?;
        Ok(())
    }

    async fn send_step_action_event_common(
        &self,
        worker_id: &Arc<String>,
        message: &AssignedAction,
        event_type: i32,
        event_payload: String,
    ) -> Result<(), crate::error::HatchetError> {
        let event = crate::worker::dispatcher::StepActionEvent {
            worker_id: worker_id.to_string(),
            job_id: message.job_id.clone(),
            job_run_id: message.job_run_id.clone(),
            step_id: message.step_id.clone(),
            step_run_id: message.step_run_id.clone(),
            action_id: message.action_id.clone(),
            event_timestamp: Some(proto_timestamp_now()),
            event_type,
            event_payload,
            retry_count: None,
            should_not_retry: None,
        };
        let request = tonic::Request::new(event);
        self.client
            .grpc_unary(request, |channel, request| async move {
                let mut client =
                    crate::worker::dispatcher::dispatcher_client::DispatcherClient::new(channel);
                client.send_step_action_event(request).await
            })
            .await?;
        Ok(())
    }
}
fn proto_timestamp_now() -> Timestamp {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    Timestamp {
        seconds: now.as_secs() as i64,
        nanos: now.subsec_nanos() as i32,
    }
}
