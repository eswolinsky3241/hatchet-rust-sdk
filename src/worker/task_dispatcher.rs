use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};

use futures::FutureExt;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::clients::client::HatchetClient;
use crate::clients::grpc::v0::dispatcher;
use crate::context::Context;
use crate::error::HatchetError;
use crate::utils::{EXECUTION_CONTEXT, ExecutionContext};
use crate::workflows::task::ExecutableTask;

#[derive(Clone)]
pub(crate) struct TaskDispatcher {
    pub(crate) registry: Arc<Mutex<HashMap<String, Arc<dyn ExecutableTask>>>>,
    pub(crate) client: HatchetClient,
    pub(crate) task_runs:
        Arc<Mutex<HashMap<String, (JoinHandle<Result<(), HatchetError>>, CancellationToken)>>>,
}

impl TaskDispatcher {
    pub(crate) async fn dispatch(
        &mut self,
        worker_id: Arc<String>,
        message: dispatcher::AssignedAction,
    ) -> Result<(), crate::HatchetError> {
        match message.action_type().as_str_name() {
            "START_STEP_RUN" => Ok(self.handle_start_step_run(worker_id, message).await?),
            "CANCEL_STEP_RUN" => Ok(self.handle_cancel_step_run(message).await?),
            _ => Err(HatchetError::UnrecognizedAction {
                action: message.action_type().as_str_name().to_string(),
            }),
        }
    }

    async fn handle_start_step_run(
        &mut self,
        worker_id: Arc<String>,
        message: dispatcher::AssignedAction,
    ) -> Result<(), crate::HatchetError> {
        let step_run_id = message.step_run_id.clone();

        self.send_step_action_event(&worker_id, &message, 1, String::from(""))
            .await?;

        let task = self
            .registry
            .lock()
            .unwrap()
            .get(&message.action_id)
            .unwrap()
            .clone();

        let token = CancellationToken::new();
        let mut client = self.client.clone();

        let execution_context = ExecutionContext {
            workflow_run_id: message.workflow_run_id.clone(),
            step_run_id: message.step_run_id.clone(),
            child_index: 0,
        };

        let context = Context::new(
            self.client.clone(),
            &message.workflow_run_id,
            &message.step_run_id,
        )
        .await;

        let handle = tokio::spawn(async move {
            EXECUTION_CONTEXT
                .scope(execution_context.into(), async move {
                    let raw_json: serde_json::Value = serde_json::from_str(&message.action_payload)
                        .expect("could not parse payload as JSON");
                    let input_value = raw_json
                        .get("input")
                        .cloned()
                        .expect("missing `input` field");

                    let result: Result<
                        Result<serde_json::Value, crate::workflows::task::TaskError>,
                        Box<dyn std::any::Any + Send>,
                    > = AssertUnwindSafe(task.execute(input_value, context))
                        .catch_unwind()
                        .await;

                    let event_payload = match &result {
                        Ok(Ok(output)) => output.to_string(),
                        Ok(Err(e)) => e.to_string(),
                        Err(panic_payload) => {
                            let panic_msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                                s.to_string()
                            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                                s.clone()
                            } else {
                                String::from("Unknown panic")
                            };
                            format!("Task panicked: {panic_msg}")
                        }
                    };

                    let event_type = if result.unwrap().is_ok() { 2 } else { 3 };

                    let event = dispatcher::StepActionEvent {
                        worker_id: worker_id.to_string(),
                        job_id: message.job_id.clone(),
                        job_run_id: message.job_run_id.clone(),
                        step_id: message.step_id.clone(),
                        step_run_id: message.step_run_id.clone(),
                        action_id: message.action_id.clone(),
                        event_timestamp: Some(crate::utils::proto_timestamp_now()?),
                        event_type,
                        event_payload,
                        retry_count: None,
                        should_not_retry: None,
                    };

                    client
                        .dispatcher_client
                        .send_step_action_event(event)
                        .await?;
                    Ok(())
                })
                .await
        });
        self.task_runs
            .lock()
            .unwrap()
            .insert(step_run_id, (handle, token));

        Ok(())
    }

    async fn handle_cancel_step_run(
        &self,
        message: dispatcher::AssignedAction,
    ) -> Result<(), crate::HatchetError> {
        let step_run_id = message.step_run_id.clone();
        if let Some((handle, token)) = self.task_runs.lock().unwrap().remove(&step_run_id) {
            println!("[{step_run_id}] Cancelling task...");
            token.cancel();
            handle.abort();
        }
        Ok(())
    }

    async fn send_step_action_event(
        &mut self,
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
            event_timestamp: Some(crate::utils::proto_timestamp_now()?),
            event_type,
            event_payload,
            retry_count: None,
            should_not_retry: None,
        };

        self.client
            .dispatcher_client
            .send_step_action_event(event)
            .await?;
        Ok(())
    }
}
