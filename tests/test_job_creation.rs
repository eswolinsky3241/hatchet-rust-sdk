use hatchet_sdk::EmptyModel;
use hatchet_sdk::HatchetClient;
use hatchet_sdk::HatchetError;
use hatchet_sdk::Worker;
use serde::{Deserialize, Serialize};
use testcontainers::{
    GenericImage, Healthcheck, ImageExt,
    core::{ExecCommand, IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};
use tokio::io::AsyncReadExt;

#[tokio::test]
async fn test_hatchet() {
    let postgres = GenericImage::new("postgres", "15.6")
        .with_exposed_port(5432.tcp())
        .with_cmd(vec!["postgres", "-c", "max_connections=200"])
        .with_health_check(
            Healthcheck::cmd(vec!["CMD-SHELL", "pg_isready -d hatchet -U hatchet"])
                .with_interval(Some(std::time::Duration::from_secs(5)))
                .with_timeout(Some(std::time::Duration::from_secs(5)))
                .with_retries(Some(5))
                .with_start_period(Some(std::time::Duration::from_secs(10))),
        )
        .with_network("hatchet")
        .with_env_var("POSTGRES_USER", "hatchet")
        .with_env_var("POSTGRES_PASSWORD", "hatchet")
        .with_env_var("POSTGRES_DB", "hatchet")
        .start()
        .await
        .unwrap();

    let hatchet = GenericImage::new("ghcr.io/hatchet-dev/hatchet/hatchet-lite", "latest")
        .with_wait_for(WaitFor::message_on_either_std(
            "created tenant 707d0855-80ab-4e1f-a156-f1c4546cbf52",
        ))
        .with_wait_for(WaitFor::Duration {
            length: std::time::Duration::from_secs(10),
        })
        .with_mapped_port(8888, 8888.tcp())
        .with_mapped_port(7077, 7077.tcp())
        .with_env_var(
            "DATABASE_URL",
            format!(
                "postgresql://hatchet:hatchet@{}:5432/hatchet?sslmode=disable",
                postgres.get_bridge_ip_address().await.unwrap()
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
    // std::thread::sleep(std::time::Duration::from_secs(30));
    let hatchet = HatchetClient::from_token(&token.trim(), "none")
        .await
        .unwrap();

    #[derive(Deserialize, Serialize, Clone)]
    struct SimpleInput {
        message: String,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    struct SimpleOutput {
        transformed_message: String,
    }

    use thiserror::Error;
    #[derive(Debug, Error)]
    pub enum MyError {
        #[error("This shit failed.")]
        Failure(#[from] HatchetError),
    }

    let my_task = hatchet.new_task(
        "step1",
        async move |input: SimpleInput,
                    ctx: hatchet_sdk::Context|
                    -> Result<SimpleOutput, MyError> {
            ctx.log(String::from("Hello from Task 1!")).await?;
            ctx.log(ctx.filter_payload().await?.to_string()).await?;
            Ok(SimpleOutput {
                transformed_message: input.message.to_lowercase(),
            })
        },
    );
    let workflow1 = hatchet
        .new_workflow::<SimpleInput, SimpleOutput>("rust-workflow3", vec![], vec![], vec![])
        .add_task(my_task)
        .unwrap();

    Worker::new("rust-worker", hatchet.clone(), 5)
        .unwrap()
        .add_workflow(workflow1)
        .start()
        .await
        .unwrap();
}

async fn start_containers_and_get_token() -> String {
    let postgres = GenericImage::new("postgres", "15.6")
        .with_exposed_port(5432.tcp())
        .with_cmd(vec!["postgres", "-c", "max_connections=200"])
        .with_health_check(
            Healthcheck::cmd(vec!["CMD-SHELL", "pg_isready -d hatchet -U hatchet"])
                .with_interval(Some(std::time::Duration::from_secs(5)))
                .with_timeout(Some(std::time::Duration::from_secs(5)))
                .with_retries(Some(5))
                .with_start_period(Some(std::time::Duration::from_secs(10))),
        )
        .with_network("hatchet")
        .with_env_var("POSTGRES_USER", "hatchet")
        .with_env_var("POSTGRES_PASSWORD", "hatchet")
        .with_env_var("POSTGRES_DB", "hatchet")
        .start()
        .await
        .unwrap();

    let hatchet = GenericImage::new("ghcr.io/hatchet-dev/hatchet/hatchet-lite", "latest")
        .with_wait_for(WaitFor::message_on_either_std(
            "created tenant 707d0855-80ab-4e1f-a156-f1c4546cbf52",
        ))
        .with_mapped_port(8888, 8888.tcp())
        .with_mapped_port(7077, 7077.tcp())
        .with_env_var(
            "DATABASE_URL",
            format!(
                "postgresql://hatchet:hatchet@{}:5432/hatchet?sslmode=disable",
                postgres.get_bridge_ip_address().await.unwrap()
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
    return token;
}
