use std::sync::Arc;

use crate::clients::grpc::v0::dispatcher;
use crate::clients::hatchet::Hatchet;
use crate::error::HatchetError;

pub(crate) struct ActionListener {
    pub(crate) client: Hatchet,
}

impl ActionListener {
    pub(crate) fn new(client: Hatchet) -> Self {
        Self { client }
    }

    pub(crate) async fn listen(
        &mut self,
        worker_id: Arc<String>,
        tx: tokio::sync::mpsc::Sender<dispatcher::AssignedAction>,
    ) -> Result<(), HatchetError> {
        let mut response = self.client.dispatcher_client.listen(&worker_id).await?;

        loop {
            match response.message().await {
                Ok(message) => match message {
                    Some(message) => {
                        tx.send(message).await.unwrap();
                    }
                    None => return Ok(()),
                },
                Err(e) => println!("{e}"),
            };
        }
    }
}
