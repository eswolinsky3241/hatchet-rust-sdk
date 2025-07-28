use std::sync::Arc;

use crate::clients::EventClient;
use crate::{HatchetClient, HatchetError};
pub struct Context {
    event_client: Arc<EventClient>,
    step_run_id: String,
}

impl Context {
    pub fn new(client: Arc<HatchetClient>, step_run_id: &str) -> Self {
        let event_client = Arc::new(EventClient::new(client.clone()));
        Self {
            event_client,
            step_run_id: step_run_id.to_string(),
        }
    }

    pub async fn log(&self, message: String) -> Result<(), HatchetError> {
        self.event_client
            .put_log(&self.step_run_id, message)
            .await?;
        Ok(())
    }
}
