use std::time::Duration;

use hatchet_sdk::{Hatchet, Register, Task, Workflow};
use serde::Serialize;
use serde::de::DeserializeOwned;

use super::containers::shared_containers;
use super::types::{SimpleInput, SimpleOutput};

const WORKER_READY_TIMEOUT: Duration = Duration::from_secs(15);
const WORKER_POLL_INTERVAL: Duration = Duration::from_millis(500);

pub struct TestHarness {
    pub hatchet: Hatchet,
    rest_base_url: String,
    rest_token: String,
    tenant_id: String,
    prefix: String,
}

impl TestHarness {
    pub async fn new(test_name: &str) -> Self {
        let containers = shared_containers().await;
        let hatchet = Hatchet::from_token(
            &containers.server_url,
            &containers.grpc_address,
            &containers.token,
            "none",
        )
        .await
        .unwrap();

        Self {
            hatchet,
            rest_base_url: containers.server_url.clone(),
            rest_token: containers.token.clone(),
            tenant_id: containers.tenant_id.clone(),
            prefix: test_name.to_string(),
        }
    }

    pub fn prefixed(&self, name: &str) -> String {
        format!("{}-{}", self.prefix, name)
    }

    pub fn simple_task(&self, name: &str) -> Task<SimpleInput, SimpleOutput> {
        self.hatchet
            .task(
                &self.prefixed(name),
                async move |input: SimpleInput,
                            _ctx: hatchet_sdk::Context|
                            -> anyhow::Result<SimpleOutput> {
                    Ok(SimpleOutput {
                        transformed_message: input.message.clone(),
                    })
                },
            )
            .build()
            .unwrap()
    }

    pub async fn spawn_worker_for_task<I, O>(&self, task: &Task<I, O>) -> WorkerGuard
    where
        I: Serialize + DeserializeOwned + Send + Sync + Clone + 'static,
        O: Serialize + DeserializeOwned + Send + Sync + Clone + 'static,
    {
        let hatchet = self.hatchet.clone();
        let task = task.clone();
        let worker_name = self.prefixed("worker");
        let handle = tokio::spawn(async move {
            hatchet
                .worker(&worker_name)
                .build()
                .unwrap()
                .add_task_or_workflow(&task)
                .start()
                .await
                .unwrap()
        });
        self.wait_for_worker_ready().await;
        WorkerGuard { handle }
    }

    pub async fn spawn_worker_for_workflow<I, O>(&self, workflow: &Workflow<I, O>) -> WorkerGuard
    where
        I: Serialize + DeserializeOwned + Send + Sync + Clone + 'static,
        O: Serialize + DeserializeOwned + Send + Sync + Clone + 'static,
    {
        let hatchet = self.hatchet.clone();
        let workflow = workflow.clone();
        let worker_name = self.prefixed("worker");
        let handle = tokio::spawn(async move {
            hatchet
                .worker(&worker_name)
                .build()
                .unwrap()
                .add_task_or_workflow(&workflow)
                .start()
                .await
                .unwrap()
        });
        self.wait_for_worker_ready().await;
        WorkerGuard { handle }
    }

    pub async fn spawn_worker_for_tasks<I, O>(&self, tasks: &[&Task<I, O>]) -> WorkerGuard
    where
        I: Serialize + DeserializeOwned + Send + Sync + Clone + 'static,
        O: Serialize + DeserializeOwned + Send + Sync + Clone + 'static,
    {
        let hatchet = self.hatchet.clone();
        let tasks: Vec<Task<I, O>> = tasks.iter().map(|t| (*t).clone()).collect();
        let worker_name = self.prefixed("worker");
        let handle = tokio::spawn(async move {
            let mut worker = hatchet.worker(&worker_name).build().unwrap();
            for task in &tasks {
                worker = worker.add_task_or_workflow(task);
            }
            worker.start().await.unwrap()
        });
        self.wait_for_worker_ready().await;
        WorkerGuard { handle }
    }

    async fn wait_for_worker_ready(&self) {
        let client = reqwest::Client::new();
        let url = format!(
            "{}/api/v1/tenants/{}/workflows",
            self.rest_base_url, self.tenant_id
        );
        let prefix_lower = self.prefix.to_lowercase();
        let deadline = tokio::time::Instant::now() + WORKER_READY_TIMEOUT;

        loop {
            if let Ok(resp) = client.get(&url).bearer_auth(&self.rest_token).send().await {
                if let Ok(body) = resp.text().await {
                    if body.to_lowercase().contains(&prefix_lower) {
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        return;
                    }
                }
            }

            if tokio::time::Instant::now() > deadline {
                panic!(
                    "Worker did not register workflow with prefix '{}' within {:?}",
                    self.prefix, WORKER_READY_TIMEOUT
                );
            }

            tokio::time::sleep(WORKER_POLL_INTERVAL).await;
        }
    }
}

pub struct WorkerGuard {
    handle: tokio::task::JoinHandle<()>,
}

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
