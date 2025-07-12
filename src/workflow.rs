use crate::client::HatchetClient;
use crate::error::HatchetError;
use serde::{Serialize, de::DeserializeOwned};
use std::fmt;
use std::marker::PhantomData;
use std::ops::Deref;

pub struct Workflow<'a, I, O> {
    name: String,
    client: &'a HatchetClient,
    _input: PhantomData<I>,
    _output: PhantomData<O>,
}

impl<'a, I, O> Workflow<'a, I, O>
where
    I: Serialize,
    O: DeserializeOwned,
{
    pub fn new(name: impl Into<String>, client: &'a HatchetClient) -> Self {
        Self {
            name: name.into(),
            client,
            _input: PhantomData,
            _output: PhantomData,
        }
    }

    pub async fn run_no_wait(
        &mut self,
        input: I,
        options: Option<TriggerWorkflowOptions>,
    ) -> Result<RunId, HatchetError> {
        self.client
            .trigger_workflow(&self.name, input, options.unwrap_or_default())
            .await
    }

    pub async fn run(
        mut self,
        input: I,
        options: Option<TriggerWorkflowOptions>,
    ) -> Result<O, HatchetError> {
        let run_id = self.run_no_wait(input, options).await?;

        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        loop {
            let wf = self.client.get_workflow(&run_id).await?;

            if let Some(task) = wf.tasks.get(0) {
                match task.status.as_str() {
                    "COMPLETED" => {
                        let output_json =
                            task.output.as_ref().ok_or(HatchetError::MissingOutput)?;
                        let output: O = serde_json::from_value(output_json.clone())
                            .map_err(|e| HatchetError::JsonDecode(e))?;
                        return Ok(output);
                    }
                    // "FAILED" => {
                    //     return Err(HatchetError::WorkflowFailed(task.clone()));
                    // }
                    _ => {
                        // still running
                    }
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunId(pub String);

impl fmt::Display for RunId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Deref for RunId {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
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
