use std::sync::Arc;

use tonic::Request;

use crate::client::HatchetClient;
use crate::error::HatchetError;
use crate::grpc::dispatcher;

pub(crate) struct ActionListener {
    pub(crate) client: Arc<HatchetClient>,
}

impl ActionListener {
    pub(crate) async fn listen(
        &self,
        worker_id: Arc<String>,
        tx: tokio::sync::mpsc::Sender<dispatcher::AssignedAction>,
    ) -> Result<(), HatchetError> {
        let request = Request::new(dispatcher::WorkerListenRequest {
            worker_id: worker_id.to_string(),
        });

        let response = self
            .client
            .grpc_stream(request, |channel, request| async move {
                let mut client = dispatcher::dispatcher_client::DispatcherClient::new(channel);
                client.listen_v2(request).await
            })
            .await?;
        let mut response = response.into_inner();
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
