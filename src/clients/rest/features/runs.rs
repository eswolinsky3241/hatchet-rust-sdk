use super::super::apis::workflow_runs_api::v1_workflow_run_get;
use crate::Configuration;
use crate::HatchetError;
use crate::clients::grpc::dispatcher_client::DispatcherClient;
use crate::clients::grpc::v0::dispatcher::ResourceEventType;
use futures::stream::Stream;
use models::*;
use std::sync::Arc;

///The runs client is a client for interacting with task and workflow runs within Hatchet.
#[derive(Clone, Debug)]
pub struct RunsClient {
    configuration: Arc<Configuration>,
    dispatcher_client: DispatcherClient,
}

impl RunsClient {
    pub(crate) fn new(
        configuration: Arc<Configuration>,
        dispatcher_client: DispatcherClient,
    ) -> Self {
        Self {
            configuration,
            dispatcher_client,
        }
    }

    /// Get a workflow run by its ID.
    ///
    /// ```no_run
    /// use hatchet_sdk::{Hatchet, EmptyModel};
    /// #[tokio::main]
    /// async fn main() {
    ///     let hatchet = Hatchet::from_env().await.unwrap();
    ///     let workflow_run = hatchet.workflow_rest_client.get("123").await.unwrap();
    /// }
    /// ```
    pub async fn get(&self, workflow_run_id: &str) -> Result<GetWorkflowRunResponse, HatchetError> {
        let response = v1_workflow_run_get(&self.configuration, workflow_run_id)
            .await
            .map_err(|e| HatchetError::RestApiError(e.to_string()))?;
        Ok(GetWorkflowRunResponse::from(response))
    }

    /// Subscribe to stream events for a workflow run. Returns an async Stream of byte chunks
    /// emitted by tasks via `ctx.put_stream()`.
    ///
    /// ```no_run
    /// use hatchet_sdk::Hatchet;
    /// use futures::StreamExt;
    /// #[tokio::main]
    /// async fn main() {
    ///     let mut hatchet = Hatchet::from_env().await.unwrap();
    ///     let mut stream = hatchet.workflow_rest_client.subscribe_to_stream("run-id").await.unwrap();
    ///     while let Some(chunk) = stream.next().await {
    ///         println!("Got chunk: {:?}", chunk.unwrap());
    ///     }
    /// }
    /// ```
    pub async fn subscribe_to_stream(
        &mut self,
        workflow_run_id: &str,
    ) -> Result<
        std::pin::Pin<Box<dyn Stream<Item = Result<Vec<u8>, HatchetError>> + Send>>,
        HatchetError,
    > {
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<Vec<u8>, HatchetError>>(100);

        let mut dispatcher = self.dispatcher_client.clone();
        let run_id = workflow_run_id.to_string();

        tokio::spawn(async move {
            let grpc_stream = match dispatcher.subscribe_to_workflow_events(&run_id).await {
                Ok(s) => s,
                Err(e) => {
                    let _ = tx.send(Err(e)).await;
                    return;
                }
            };

            let mut grpc_stream = grpc_stream;
            loop {
                match grpc_stream.message().await {
                    Ok(Some(event)) => {
                        if event.hangup {
                            break;
                        }
                        if event.event_type == ResourceEventType::Stream as i32 {
                            let payload = event.event_payload.into_bytes();
                            if tx.send(Ok(payload)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        let _ = tx
                            .send(Err(HatchetError::GrpcErrorStatus(e.message().to_string())))
                            .await;
                        break;
                    }
                }
            }
        });

        Ok(Box::pin(futures::stream::unfold(rx, |mut rx| async {
            rx.recv().await.map(|item| (item, rx))
        })))
    }
}

pub mod models {
    use crate::clients::rest::models::V1WorkflowRunCreate200Response;
    use serde::Deserialize;
    use serde_json::Value;
    use std::collections::HashMap;

    #[derive(Debug, Deserialize)]
    pub struct GetWorkflowRunResponse {
        pub tasks: Vec<Task>,
        pub run: Run,
    }

    #[derive(Debug, Deserialize)]
    pub struct TaskParent(pub Value);

    #[derive(Debug, Deserialize)]
    pub struct Triggers {
        pub filter_payload: serde_json::Value,
    }

    #[derive(Debug, Deserialize)]
    pub struct TaskInput {
        pub parents: HashMap<String, TaskParent>,
        pub triggers: Triggers,
    }

    #[derive(Debug, Deserialize)]
    pub struct Task {
        pub output: Option<Value>,
        pub input: TaskInput,
        #[serde(rename = "taskExternalId")]
        pub task_external_id: String,
        #[serde(rename = "actionId")]
        pub action_id: Option<String>,
    }

    #[derive(Debug, Deserialize)]
    pub struct Run {
        pub status: WorkflowStatus,
        #[serde(rename = "errorMessage")]
        pub error_message: String,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "UPPERCASE")]
    pub enum WorkflowStatus {
        Running,
        Failed,
        Completed,
        Queued,
        Cancelled,
        #[serde(other)]
        Unknown,
    }

    #[derive(Debug, Deserialize)]
    pub struct Workflow;

    impl From<V1WorkflowRunCreate200Response> for GetWorkflowRunResponse {
        fn from(response: V1WorkflowRunCreate200Response) -> Self {
            let json_str = serde_json::to_string(&response)
                .expect("Failed to serialize V1WorkflowRunCreate200Response");
            serde_json::from_str(&json_str)
                .expect("Failed to deserialize to GetWorkflowRunResponse")
        }
    }
}
