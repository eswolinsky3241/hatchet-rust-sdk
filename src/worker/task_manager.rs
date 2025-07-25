// use std::collections::HashMap;
// use std::sync::Arc;

// use tokio::task::JoinHandle;
// use tokio_util::sync::CancellationToken;

// use crate::client::HatchetClient;
// use crate::grpc::dispatcher;
// use crate::tasks::ErasedTask;

// pub struct TaskManager {
//     pub task_registry: Arc<HashMap<String, Arc<dyn ErasedTask>>>
//     pub task_run_registry: HashMap<String, CancellationToken>,
//     pub rx: tokio::sync::mpsc::Receiver<dispatcher::AssignedAction>,
// }

// impl TaskManager {
//     pub async fn subscribe(&self, client: HatchetClient, worker_id: &str) -> Result<(), ()> {
//         let dispatcher = Arc::new(crate::worker::task_dispatcher::TaskDispatcher {
//             registry: self.task_registry,
//             client: Arc::clone(client),
//         });

//         while let Some(task) = self.rx.recv().await {
//             let worker_id = Arc::clone(worker_id);
//             let dispatcher = dispatcher.clone();
//             tokio::spawn(async move {
//                 if let Err(e) = dispatcher.dispatch(worker_id, task).await {
//                     eprintln!("Task failed: {e}");
//                 }
//             });
//         }
//         Ok(())
//     }
//     pub async fn spawn() {}

//     pub async fn cancel() {}
// }
