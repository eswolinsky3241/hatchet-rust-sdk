use std::env;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::error::HatchetError;

#[derive(Debug, Clone)]
pub struct HatchetConfig {
    pub api_token: String,
    pub grpc_address: String,
    pub server_url: String,
}

impl HatchetConfig {
    pub fn from_env() -> Result<Self, HatchetError> {
        let token = env::var("HATCHET_CLIENT_TOKEN").map_err(|_| HatchetError::MissingEnvVar {
            var: "HATCHET_CLIENT_TOKEN".to_string(),
        })?;
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(HatchetError::InvalidTokenFormat);
        }

        let payload_bytes = URL_SAFE_NO_PAD.decode(parts[1])?;
        let payload_json: serde_json::Value = serde_json::from_slice(&payload_bytes)?;

        let grpc_address = payload_json
            .get("grpc_broadcast_address")
            .and_then(|v| v.as_str())
            .ok_or(HatchetError::MissingGrpcAddress)?;

        let server_url = payload_json
            .get("server_url")
            .and_then(|v| v.as_str())
            .ok_or(HatchetError::MissingServerUrl)?;

        Ok(Self {
            api_token: token,
            grpc_address: grpc_address.to_string(),
            server_url: server_url.to_string(),
        })
    }
}
