use std::sync::Mutex;

use testcontainers::core::wait::HealthWaitStrategy;
use testcontainers::{
    GenericImage, Healthcheck, ImageExt,
    core::ContainerAsync,
    core::{ExecCommand, IntoContainerPort, WaitFor},
    runners::AsyncRunner,
};
use tokio::io::AsyncReadExt;
use tokio::sync::OnceCell;

pub struct SharedContainers {
    pub server_url: String,
    pub grpc_address: String,
    pub token: String,
    pub tenant_id: String,
    #[allow(dead_code)]
    containers: ContainerGroup,
}

#[allow(dead_code)]
struct ContainerGroup {
    postgres: ContainerAsync<GenericImage>,
    hatchet: ContainerAsync<GenericImage>,
}

static INSTANCE: OnceCell<SharedContainers> = OnceCell::const_new();
static CONTAINER_IDS: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub async fn shared_containers() -> &'static SharedContainers {
    INSTANCE.get_or_init(|| SharedContainers::start()).await
}

fn register_cleanup(ids: Vec<String>) {
    if let Ok(mut stored) = CONTAINER_IDS.lock() {
        *stored = ids;
    }

    extern "C" fn cleanup() {
        if let Ok(ids) = CONTAINER_IDS.lock() {
            if !ids.is_empty() {
                let mut cmd = std::process::Command::new("docker");
                cmd.arg("rm").arg("-f");
                for id in ids.iter() {
                    cmd.arg(id);
                }
                let _ = cmd.output();
            }
        }
    }

    unsafe {
        libc::atexit(cleanup);
    }
}

impl SharedContainers {
    async fn start() -> Self {
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

        let hatchet_lite_version = hatchet_lite_version();

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

        let postgres_id = postgres.id().to_string();
        let hatchet_id = hatchet.id().to_string();
        register_cleanup(vec![hatchet_id, postgres_id]);

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

        let server_url = format!(
            "http://localhost:{}",
            hatchet.get_host_port_ipv4(8888).await.unwrap()
        );
        let grpc_address = format!(
            "localhost:{}",
            hatchet.get_host_port_ipv4(7077).await.unwrap()
        );

        Self {
            server_url,
            grpc_address,
            token: token.trim().to_string(),
            tenant_id: "707d0855-80ab-4e1f-a156-f1c4546cbf52".to_string(),
            containers: ContainerGroup { postgres, hatchet },
        }
    }
}

fn hatchet_lite_version() -> String {
    std::env::var("TEST_HATCHET_LITE_VERSION").unwrap_or("latest".to_string())
}
