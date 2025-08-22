use std::sync::Arc;

use crate::client::HatchetClientTrait;
use crate::error::HatchetError;
use crate::grpc::v0::dispatcher;

pub(crate) struct ActionListener {
    pub(crate) client: Box<dyn HatchetClientTrait>,
}

impl ActionListener {
    pub(crate) fn new(client: Box<dyn HatchetClientTrait>) -> Self {
        Self { client }
    }

    pub(crate) async fn listen(
        &mut self,
        worker_id: Arc<String>,
        tx: tokio::sync::mpsc::Sender<dispatcher::AssignedAction>,
    ) -> Result<(), HatchetError> {
        let mut response = self.client.listen(&worker_id).await?;

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
