use crate::client::HatchetClient;
use crate::error::HatchetError;
use serde::Serialize;

pub struct Workflow<'a> {
    name: &'a str,
    client: &'a HatchetClient,
}

impl<'a> Workflow<'a> {
    pub fn new(name: &'a str, client: &'a HatchetClient) -> Self {
        Self { name, client }
    }

    pub async fn run_no_wait<T>(&self, input: T) -> Result<String, HatchetError>
    where
        T: Serialize,
    {
        self.client.run_no_wait(self.name, input).await
    }
}
