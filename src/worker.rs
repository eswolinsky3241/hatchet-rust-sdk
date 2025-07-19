use dispatcher::{HeartbeatRequest, HeartbeatResponse, dispatcher_server};

use crate::client::HatchetClient;
use crate::error::HatchetError;
use crate::workflow::Workflow;

pub mod dispatcher {
    tonic::include_proto!("_");
}

pub struct Worker<'a> {
    pub name: String,
    pub id: String,
    pub client: &'a HatchetClient,
}

impl<'a> Worker<'a> {
    pub fn register_workflow<I, O>(workflow: Workflow<I, O>) -> Result<(), HatchetError> {
        Ok(())
    }

    pub async fn start(&self) -> Result<(), HatchetError> {
        loop {
            self.client.heartbeat(&self.id).await?;
            tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
        }
    }

    pub async fn heartbeat() -> Result<(), HatchetError> {
        // dispatcher_server
        Ok(())
    }

    pub async fn register() -> Result<(), HatchetError> {
        Ok(())
    }
}
