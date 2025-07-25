use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use prost_types::Timestamp;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::error::HatchetError;
use crate::grpc::dispatcher;
use crate::tasks::ErasedTask;

pub struct TaskDispatcher {
    pub registry: Arc<HashMap<String, Arc<dyn ErasedTask>>>,
    pub client: Arc<crate::client::HatchetClient>,
    pub task_runs: Arc<Mutex<HashMap<String, (JoinHandle<()>, CancellationToken)>>>,
}

impl TaskDispatcher {
    pub async fn dispatch(
        &self,
        worker_id: Arc<String>,
        message: dispatcher::AssignedAction,
    ) -> Result<(), crate::error::HatchetError> {
        match message.action_type().as_str_name() {
            "START_STEP_RUN" => {
                self.handle_start_step_run(worker_id, message).await?;
            }
            "CANCEL_STEP_RUN" => {
                let step_run_id = message.step_run_id.clone();
                if let Some((handle, token)) = self.task_runs.lock().unwrap().remove(&step_run_id) {
                    println!("[{step_run_id}] Cancelling task...");
                    token.cancel();
                    handle.abort();
                } else {
                    println!("[{step_run_id}] No running task found to cancel.");
                }
            }
            _ => println!("GOT SOMETHING ELSE"),
        }
        Ok(())
    }

    async fn handle_start_step_run(
        &self,
        worker_id: Arc<String>,
        message: dispatcher::AssignedAction,
    ) -> Result<(), crate::error::HatchetError> {
        let step_run_id = message.step_run_id.clone();

        self.send_step_action_event_common(&worker_id, &message, 1, "".to_string())
            .await?;

        let handler = self
            .registry
            .get(&message.action_id)
            .expect("handler not found")
            .clone();

        let token = CancellationToken::new();
        let client = self.client.clone();
        let worker_id_clone = worker_id.clone();

        let handle = tokio::spawn(async move {
            let raw_json: serde_json::Value = serde_json::from_str(&message.action_payload)
                .expect("could not parse payload as JSON");
            let input_value = raw_json
                .get("input")
                .cloned()
                .expect("missing `input` field");

            let result = handler.run_from_json(input_value).await;

            let event_payload = match &result {
                Ok(json) => json.to_string(),
                Err(e) => format!("Task error: {e}"),
            };

            let event_type = if result.is_ok() { 2 } else { 3 };

            let event = dispatcher::StepActionEvent {
                worker_id: worker_id_clone.to_string(),
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
            let _ = client
                .grpc_unary(request, |channel, request| async move {
                    let mut client = dispatcher::dispatcher_client::DispatcherClient::new(channel);
                    client.send_step_action_event(request).await
                })
                .await;
        });

        self.task_runs
            .lock()
            .unwrap()
            .insert(step_run_id, (handle, token));

        Ok(())
    }

    async fn send_step_action_event_common(
        &self,
        worker_id: &Arc<String>,
        message: &dispatcher::AssignedAction,
        event_type: i32,
        event_payload: String,
    ) -> Result<(), HatchetError> {
        let event = dispatcher::StepActionEvent {
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
                let mut client = dispatcher::dispatcher_client::DispatcherClient::new(channel);
                client.send_step_action_event(request).await
            })
            .await?;
        Ok(())
    }
}

fn proto_timestamp_now() -> Timestamp {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();

    Timestamp {
        seconds: now.as_secs() as i64,
        nanos: now.subsec_nanos() as i32,
    }
}
