use std::collections::HashMap;
use std::panic::AssertUnwindSafe;
use std::sync::{Arc, Mutex};

use futures::FutureExt;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::clients::grpc::v0::dispatcher;
use crate::clients::hatchet::Hatchet;
use crate::context::Context;
use crate::error::HatchetError;
use crate::runnables::ExecutableTask;
use crate::utils::{EXECUTION_CONTEXT, ExecutionContext};

pub(crate) struct TaskRunEntry {
    handle: Option<JoinHandle<Result<(), HatchetError>>>,
    token: CancellationToken,
}

type TaskRun = Arc<Mutex<HashMap<String, TaskRunEntry>>>;

#[derive(Clone)]
pub(crate) struct TaskDispatcher {
    pub(crate) registry: Arc<Mutex<HashMap<String, Arc<dyn ExecutableTask>>>>,
    pub(crate) client: Hatchet,
    pub(crate) task_runs: TaskRun,
}

impl TaskDispatcher {
    pub(crate) async fn dispatch(
        &mut self,
        worker_id: Arc<String>,
        message: dispatcher::AssignedAction,
    ) -> Result<(), crate::HatchetError> {
        match message.action_type().as_str_name() {
            "START_STEP_RUN" => {
                log::debug!(
                    "start step run: {}/{}",
                    message.action_id,
                    message.task_run_external_id
                );
                Ok(self.handle_start_step_run(worker_id, message).await?)
            }
            "CANCEL_STEP_RUN" => {
                log::info!(
                    "cancel step run: {}/{}",
                    message.action_id,
                    message.task_run_external_id
                );
                Ok(self.handle_cancel_step_run(message).await?)
            }
            _ => Err(HatchetError::UnrecognizedAction(
                message.action_type().as_str_name().to_string(),
            )),
        }
    }

    async fn handle_start_step_run(
        &mut self,
        worker_id: Arc<String>,
        message: dispatcher::AssignedAction,
    ) -> Result<(), crate::HatchetError> {
        let task_run_external_id = message.task_run_external_id.clone();

        self.send_step_action_event(&worker_id, &message, 1, String::from(""))
            .await?;

        let task = self
            .registry
            .lock()
            .expect("failed to acquire lock on task runs")
            .get(&message.action_id)
            .ok_or(HatchetError::TaskNotFound {
                task_name: message.action_id.clone(),
            })?
            .clone();

        let token = CancellationToken::new();
        let mut client = self.client.clone();

        let execution_context = ExecutionContext {
            workflow_run_id: message.workflow_run_id.clone(),
            task_run_external_id: message.task_run_external_id.clone(),
            child_index: 0,
        };

        let context = Context::new(
            self.client.clone(),
            &message.workflow_run_id,
            &message.task_run_external_id,
        );

        let task_runs_cleanup = self.task_runs.clone();
        let cleanup_id = task_run_external_id.clone();

        // Insert entry BEFORE spawning to prevent a race condition where the
        // spawned task completes and calls remove() before insert() runs,
        // leaving a permanent orphaned entry in the HashMap.
        //
        // Note: if a cancellation arrives between this insert and the spawn below,
        // the entry is removed and the token is cancelled, but there is no JoinHandle
        // to abort yet. The spawned task will still observe the cancelled token via
        // cooperative cancellation. This window is narrow and acceptable.
        self.task_runs
            .lock()
            .expect("failed to acquire lock on task runs")
            .insert(
                task_run_external_id.clone(),
                TaskRunEntry {
                    handle: None,
                    token: token.clone(),
                },
            );

        let handle = tokio::spawn(async move {
            let result = EXECUTION_CONTEXT
                .scope(execution_context.into(), async move {
                    let raw_json: serde_json::Value = serde_json::from_str(&message.action_payload)
                        .expect("could not parse payload as JSON");
                    let input_value = raw_json
                        .get("input")
                        .cloned()
                        .expect("missing `input` field");

                    let result: Result<
                        Result<serde_json::Value, crate::runnables::TaskError>,
                        Box<dyn std::any::Any + Send>,
                    > = AssertUnwindSafe(task.execute(input_value, context))
                        .catch_unwind()
                        .await;

                    let event_payload = match &result {
                        Ok(Ok(output)) => {
                            log::info!(
                                "finished step run {}/{}",
                                message.action_id,
                                message.task_run_external_id
                            );
                            (2, output.to_string())
                        }
                        Ok(Err(e)) => {
                            log::error!(
                                "error returned in action ({}, retry={})",
                                message.action_id,
                                message.retry_count
                            );
                            (3, e.to_string())
                        }
                        Err(panic_payload) => {
                            log::error!(
                                "panic raised in action ({}, retry={})",
                                message.action_id,
                                message.retry_count
                            );
                            let panic_msg = if let Some(s) = panic_payload.downcast_ref::<&str>() {
                                s.to_string()
                            } else if let Some(s) = panic_payload.downcast_ref::<String>() {
                                s.clone()
                            } else {
                                String::from("Unknown panic")
                            };
                            (3, format!("Task panicked: {panic_msg}"))
                        }
                    };

                    let event = dispatcher::StepActionEvent {
                        worker_id: worker_id.to_string(),
                        job_id: message.job_id.clone(),
                        job_run_id: message.job_run_id.clone(),
                        task_id: message.task_id.clone(),
                        task_run_external_id: message.task_run_external_id.clone(),
                        action_id: message.action_id.clone(),
                        event_timestamp: Some(crate::utils::proto_timestamp_now()?),
                        event_type: event_payload.0,
                        event_payload: event_payload.1,
                        retry_count: None,
                        should_not_retry: None,
                    };

                    client
                        .dispatcher_client
                        .send_step_action_event(event)
                        .await?;
                    Ok(())
                })
                .await;

            // Remove completed task run from tracking map to prevent memory leak.
            task_runs_cleanup
                .lock()
                .expect("failed to acquire lock on task runs")
                .remove(&cleanup_id);

            result
        });

        // Update with the real JoinHandle. The entry may already have been
        // removed if the task completed before we get here — that's fine.
        if let Some(entry) = self
            .task_runs
            .lock()
            .expect("failed to acquire lock on task runs")
            .get_mut(&task_run_external_id)
        {
            entry.handle = Some(handle);
        }

        Ok(())
    }

    async fn handle_cancel_step_run(
        &self,
        message: dispatcher::AssignedAction,
    ) -> Result<(), crate::HatchetError> {
        let task_run_external_id = message.task_run_external_id.clone();
        if let Some(entry) = self
            .task_runs
            .lock()
            .expect("failed to acquire lock on task runs")
            .remove(&task_run_external_id)
        {
            entry.token.cancel();
            if let Some(handle) = entry.handle {
                handle.abort();
            }
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
            task_id: message.task_id.clone(),
            task_run_external_id: message.task_run_external_id.clone(),
            action_id: message.action_id.clone(),
            event_timestamp: Some(crate::utils::proto_timestamp_now()?),
            event_type,
            event_payload,
            retry_count: Some(message.retry_count),
            should_not_retry: None,
        };

        self.client
            .dispatcher_client
            .send_step_action_event(event)
            .await?;
        Ok(())
    }
}
