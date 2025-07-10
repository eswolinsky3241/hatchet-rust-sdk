use crate::client::HatchetClient;
use crate::error::HatchetError;
use serde::{Serialize, de::DeserializeOwned};
use std::marker::PhantomData;

pub struct Task<'a, I, O> {
    name: String,
    client: &'a mut HatchetClient,
    _input: PhantomData<I>,
    _output: PhantomData<O>,
}

impl<'a, I, O> Task<'a, I, O>
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

    pub async fn run_no_wait(&mut self, input: I) -> Result<String, HatchetError> {
        self.client.run_no_wait(&self.name, input).await
    }
}
