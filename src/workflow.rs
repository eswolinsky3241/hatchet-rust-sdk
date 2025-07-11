use crate::client::HatchetClient;
use crate::error::HatchetError;
use serde::{Serialize, de::DeserializeOwned};
use std::marker::PhantomData;

pub struct Workflow<'a, I, O> {
    name: String,
    client: &'a mut HatchetClient,
    _input: PhantomData<I>,
    _output: PhantomData<O>,
}

impl<'a, I, O> Workflow<'a, I, O>
where
    I: Serialize,
    O: DeserializeOwned,
{
    pub fn new(name: impl Into<String>, client: &'a mut HatchetClient) -> Self {
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
        options: TriggerWorkflowOptions,
    ) -> Result<RunId, HatchetError> {
        self.client.trigger_workflow(&self.name, input).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunId(pub String);

#[derive(Debug, Default, Clone)]
pub struct TriggerWorkflowOptions {
    pub additional_metadata: Option<serde_json::Value>,
    pub desired_worker_id: Option<String>,
    pub namespace: Option<String>,
    pub sticky: bool,
    pub key: Option<String>,
}
