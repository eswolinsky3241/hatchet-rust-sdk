use std::sync::Arc;

use crate::client::HatchetClientTrait;
use crate::error::HatchetError;
use crate::grpc::v0::dispatcher;

pub(crate) struct ActionListener<C> {
    pub(crate) client: Arc<tokio::sync::Mutex<C>>,
}

impl<C> ActionListener<C>
where
    C: HatchetClientTrait,
{
    pub(crate) fn new(client: Arc<tokio::sync::Mutex<C>>) -> Self {
        Self { client }
    }

    pub(crate) async fn listen(
        &self,
        worker_id: Arc<String>,
        tx: tokio::sync::mpsc::Sender<dispatcher::AssignedAction>,
    ) -> Result<(), HatchetError> {
        let mut response = self.client.lock().await.listen(&worker_id).await?;

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
