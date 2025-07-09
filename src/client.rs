#![cfg(feature = "client")]

use crate::config::HatchetConfig;
use crate::error::HatchetError;
use serde::Serialize;

#[derive(Debug)]
pub struct HatchetClient {
    pub config: HatchetConfig,
}

impl HatchetClient {
    pub fn new(config: HatchetConfig) -> Self {
        Self { config }
    }
}

impl HatchetClient {
    pub async fn run_no_wait<T>(&self, task_name: &str, input: T) -> Result<String, HatchetError>
    where
        T: Serialize,
    {
        // serialize input and send to your gRPC or REST endpoint
        unimplemented!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
}
