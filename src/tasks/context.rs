use std::sync::Arc;

use tokio::sync::mpsc;

use crate::clients::EventClient;
use crate::{HatchetClient, HatchetError};
pub struct Context {
    logger_tx: mpsc::Sender<String>,
}

impl Context {
    pub fn new(client: Arc<HatchetClient>, step_run_id: &str) -> Self {
        let event_client = Arc::new(EventClient::new(client.clone()));
        let (tx, mut rx) = mpsc::channel::<String>(100);
        let step_run_id = step_run_id.to_string();
        tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                event_client.put_log(&step_run_id, message).await.unwrap();
            }
        });
        Self { logger_tx: tx }
    }

    pub async fn run_logger_thread(&self) -> () {}

    pub async fn log(&self, message: String) -> Result<(), HatchetError> {
        self.logger_tx.send(message).await.unwrap();

        Ok(())
    }
}
