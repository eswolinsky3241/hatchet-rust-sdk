use serde::{Deserialize, Serialize};
use testcontainers::core::wait::HealthWaitStrategy;
use testcontainers::{
    GenericImage, Healthcheck, ImageExt,
    core::ContainerAsync,
    core::{ExecCommand, IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};
use thiserror::Error;
use tokio::io::AsyncReadExt;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SimpleInput {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SimpleOutput {
    pub transformed_message: String,
}

#[derive(Clone, Debug, Error)]
pub enum MyError {
    #[error("Test failed.")]
    Failure,
}

pub(crate) async fn start_containers_and_get_token() -> (
    ContainerAsync<GenericImage>,
    ContainerAsync<GenericImage>,
    String,
) {
    let postgres = GenericImage::new("postgres", "15.6")
        .with_exposed_port(5432.tcp())
        .with_cmd(vec!["postgres", "-c", "max_connections=200"])
        .with_health_check(
            Healthcheck::cmd(vec!["CMD-SHELL", "pg_isready -d hatchet -U hatchet"])
                .with_interval(Some(std::time::Duration::from_secs(5)))
                .with_timeout(Some(std::time::Duration::from_secs(5)))
                .with_retries(Some(5)),
        )
        .with_network("hatchet")
        .with_env_var("POSTGRES_USER", "hatchet")
        .with_env_var("POSTGRES_PASSWORD", "hatchet")
        .with_env_var("POSTGRES_DB", "hatchet")
        .start()
        .await
        .unwrap();

    let hatchet_lite_version = get_hatchet_lite_version();

    let hatchet = GenericImage::new(
        "ghcr.io/hatchet-dev/hatchet/hatchet-lite",
        &hatchet_lite_version,
    )
    .with_exposed_port(8888.tcp())
    .with_exposed_port(7077.tcp())
    .with_wait_for(WaitFor::message_on_either_std(
        "created tenant 707d0855-80ab-4e1f-a156-f1c4546cbf52",
    ))
    .with_wait_for(WaitFor::Healthcheck(HealthWaitStrategy::new()))
    .with_health_check(
        Healthcheck::cmd(vec!["nc", "-z", "localhost", "7077"])
            .with_interval(Some(std::time::Duration::from_secs(5)))
            .with_timeout(Some(std::time::Duration::from_secs(5)))
            .with_retries(Some(5)),
    )
    .with_env_var(
        "DATABASE_URL",
        format!(
            "postgresql://hatchet:hatchet@{}:5432/hatchet?sslmode=disable",
            postgres.get_bridge_ip_address().await.unwrap(),
        ),
    )
    .with_network("hatchet")
    .with_env_var("SERVER_AUTH_COOKIE_DOMAIN", "localhost")
    .with_env_var("SERVER_AUTH_COOKIE_INSECURE", "t")
    .with_env_var("SERVER_GRPC_BIND_ADDRESS", "0.0.0.0")
    .with_env_var("SERVER_GRPC_INSECURE", "t")
    .with_env_var("SERVER_GRPC_BROADCAST_ADDRESS", "localhost:7077")
    .with_env_var("SERVER_GRPC_PORT", "7077")
    .with_env_var("SERVER_URL", "http://localhost:8888")
    .with_env_var("SERVER_AUTH_SET_EMAIL_VERIFIED", "t")
    .with_env_var("SEVER_DEFAULT_ENGINE_VERSION", "V1")
    .with_env_var(
        "SERVER_INTERNAL_CLIENT_INTERNAL_GRPC_BROADCAST_ADDRESS",
        "localhost:7077",
    )
    .start()
    .await
    .unwrap();

    let command = ExecCommand::new(vec![
        "/hatchet-admin",
        "token",
        "create",
        "--config",
        "/config",
        "--tenant-id",
        "707d0855-80ab-4e1f-a156-f1c4546cbf52",
    ]);
    let mut result = hatchet.exec(command).await.unwrap();
    let mut token = String::new();
    result.stdout().read_to_string(&mut token).await.unwrap();
    (postgres, hatchet, token)
}

fn get_hatchet_lite_version() -> String {
    use std::env;

    env::var("TEST_HATCHET_LITE_VERSION").unwrap_or_else(|_| "latest".to_string())
}
