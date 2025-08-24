use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::client::HatchetClientTrait;
use crate::error::HatchetError;
use crate::grpc::v1::workflows::{
    CreateTaskOpts, CreateWorkflowVersionRequest, DefaultFilter as DefaultFilterProto,
};
use crate::rest::models::WorkflowStatus;

use crate::workflows::task::{ExecutableTask, Task};

#[derive(Clone)]
pub struct Workflow<I, O> {
    pub(crate) name: String,
    client: Box<dyn HatchetClientTrait>,
    pub(crate) executable_tasks: Vec<Box<dyn ExecutableTask>>,
    tasks: Vec<CreateTaskOpts>,
    on_events: Vec<String>,
    cron_triggers: Vec<String>,
    default_filters: Vec<DefaultFilter>,
    _phantom: std::marker::PhantomData<(I, O)>,
}

impl<I, O> Workflow<I, O>
where
    I: Serialize + Send + Sync,
    O: DeserializeOwned + Send + Sync,
{
    pub fn new(
        name: impl Into<String>,
        client: Box<dyn HatchetClientTrait>,
        on_events: Vec<String>,
        cron_triggers: Vec<String>,
        default_filters: Vec<DefaultFilter>,
    ) -> Self {
        Self {
            name: name.into(),
            client,
            executable_tasks: vec![],
            tasks: vec![],
            on_events,
            cron_triggers: cron_triggers,
            default_filters,
            _phantom: std::marker::PhantomData,
        }
    }

    pub fn add_task<P, E>(mut self, task: Task<I, P, E>) -> Result<Self, HatchetError>
    where
        I: serde::de::DeserializeOwned + Send + 'static,
        P: serde::Serialize + Send + 'static,
        E: Into<Box<dyn std::error::Error + Send + Sync>> + Send + 'static,
    {
        if self
            .tasks
            .iter()
            .any(|existing_task| existing_task.readable_id == task.name)
        {
            return Err(HatchetError::DuplicateTask {
                task_name: task.name.clone(),
                workflow_name: self.name.clone(),
            });
        }

        self.tasks.push(task.to_proto(&self.name));
        self.executable_tasks.push(task.into_executable());
        Ok(self)
    }

    pub(crate) fn to_proto(&self) -> CreateWorkflowVersionRequest {
        CreateWorkflowVersionRequest {
            name: self.name.clone(),
            description: String::from(""),
            version: String::from(""),
            event_triggers: self.on_events.clone(),
            cron_triggers: self.cron_triggers.clone(),
            tasks: self.tasks.clone(),
            concurrency: None,
            cron_input: None,
            on_failure_task: None,
            sticky: None,
            default_priority: None,
            concurrency_arr: vec![],
            default_filters: self
                .default_filters
                .clone()
                .into_iter()
                .map(|f| f.to_proto())
                .collect(),
        }
    }

    pub async fn run_no_wait(
        &mut self,
        input: I,
        options: Option<TriggerWorkflowOptions>,
    ) -> Result<String, HatchetError> {
        self.trigger(input, options.unwrap_or_default()).await
    }

    pub async fn run(
        &mut self,
        input: I,
        options: Option<TriggerWorkflowOptions>,
    ) -> Result<O, HatchetError> {
        let run_id = self.run_no_wait(input, options).await?;

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        loop {
            let workflow = self.get_run(&run_id).await?;

            match workflow.run.status {
                WorkflowStatus::Running => {}
                WorkflowStatus::Completed => {
                    let output_json = workflow
                        .tasks
                        .last() // Get the output of the last task
                        .ok_or(HatchetError::MissingTasks)?
                        .output
                        .as_ref()
                        .ok_or(HatchetError::MissingOutput)?;
                    let output: O = serde_json::from_value(output_json.clone())
                        .map_err(|e| HatchetError::JsonDecodeError(e))?;
                    return Ok(output);
                }
                WorkflowStatus::Failed => {
                    return Err(HatchetError::WorkflowFailed {
                        error_message: workflow.run.error_message.clone(),
                    });
                }
                _ => {}
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }

    async fn trigger<T>(
        &mut self,
        input: T,
        options: TriggerWorkflowOptions,
    ) -> Result<String, HatchetError>
    where
        T: Serialize,
    {
        let input_json = serde_json::to_value(&input).map_err(HatchetError::JsonEncode)?;

        let response = self
            .client
            .trigger_workflow(&self.name, input_json, options.additional_metadata)
            .await?;

        Ok(response.workflow_run_id)
    }

    async fn get_run(
        &self,
        run_id: &str,
    ) -> Result<crate::rest::models::GetWorkflowRunResponse, HatchetError> {
        self.client.get_workflow_run(run_id).await
    }
}

#[derive(Debug, Default, Clone)]
pub struct TriggerWorkflowOptions {
    pub additional_metadata: Option<serde_json::Value>,
    pub desired_worker_id: Option<String>,
    pub namespace: Option<String>,
    pub sticky: bool,
    pub key: Option<String>,
}

#[derive(Debug, Default, Clone)]
pub struct DefaultFilter {
    pub expression: String,
    pub scope: String,
    pub payload: Option<serde_json::Value>,
}

impl DefaultFilter {
    pub fn new(expression: String, scope: String, payload: Option<serde_json::Value>) -> Self {
        Self {
            expression,
            scope,
            payload,
        }
    }
}

impl DefaultFilter {
    pub fn to_proto(&self) -> DefaultFilterProto {
        DefaultFilterProto {
            expression: self.expression.clone(),
            scope: self.scope.clone(),
            payload: self.payload.clone().map(|v| v.to_string().into()),
        }
    }
}

// #[cfg(test)]
// mod tests {

//     use super::*;
//     use crate::EmptyModel;
//     use crate::config::HatchetConfig;

//     #[tokio::test]
//     async fn test_duplicate_task_names_raises_error() {
//         let payload = "eyJzZXJ2ZXJfdXJsIjoiaHR0cHM6Ly9oYXRjaGV0LmNvbSIsImdycGNfYnJvYWRjYXN0X2FkZHJlc3MiOiJlbmdpbmUuaGF0Y2hldC5jb20ifQ";
//         let token = format!("header.{}.sig", payload.to_string());
//         let config = HatchetConfig::new(&token, "none").unwrap();
//         let client = HatchetClient::new(config).await.unwrap();
//         let workflow =
//             Workflow::<EmptyModel, EmptyModel>::new("test-workflow", client, vec![], vec![]);

//         let task1: Task<_, _> = Task::<EmptyModel, EmptyModel>::new(
//             "test-task",
//             |_input: EmptyModel, _ctx: crate::Context| async move { Ok(EmptyModel {}) },
//         );

//         let task2: Task<_, _> = Task::<EmptyModel, EmptyModel>::new(
//             "test-task",
//             |_input: EmptyModel, _ctx: crate::Context| async move { Ok(EmptyModel {}) },
//         );

//         assert!(matches!(
//             workflow.add_task(task1).unwrap().add_task(task2),
//             Err(HatchetError::DuplicateTask {
//                 task_name: _,
//                 workflow_name: _
//             })
//         ));
//     }

//     // #[tokio::test]
//     // async fn test_run_no_wait_returns_run_id() {
//     //     use std::sync::Arc;

//     //     use crate::client::MockHatchetClientTrait;
//     //     use crate::grpc::v0::workflows::TriggerWorkflowResponse;

//     //     let mut mock_client = MockHatchetClientTrait::new();
//     //     let expected_run_id = "test-run-id-12345";

//     //     mock_client
//     //         .expect_trigger_workflow()
//     //         .times(1)
//     //         .returning(move |_request| {
//     //             Ok(TriggerWorkflowResponse {
//     //                 workflow_run_id: expected_run_id.to_string(),
//     //             })
//     //         });

//     //     let mock_client = Arc::new(mock_client);

//     //     let workflow = Workflow::<EmptyModel, EmptyModel, _>::new(
//     //         "test-workflow",
//     //         mock_client,
//     //         vec![],
//     //         vec![],
//     //     );

//     //     let run_id = workflow.run_no_wait(EmptyModel {}, None).await.unwrap();
//     //     assert_eq!(run_id, expected_run_id);
//     // }
// }
