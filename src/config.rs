use std::env;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;

use crate::error::HatchetError;

#[derive(Debug, Clone)]
pub struct HatchetConfig {
    pub(crate) api_token: String,
    pub(crate) grpc_address: String,
    pub(crate) server_url: String,
}

impl HatchetConfig {
    pub fn new(token: String) -> Result<Self, HatchetError> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(HatchetError::InvalidTokenFormat);
        }

        let payload_json = Self::decode_token(&parts[1])?;

        let (grpc_address, server_url) = Self::parse_token(payload_json)?;

        Ok(Self {
            api_token: token,
            grpc_address: grpc_address.to_string(),
            server_url: server_url.to_string(),
        })
    }

    pub fn from_env() -> Result<Self, HatchetError> {
        let token = env::var("HATCHET_CLIENT_TOKEN").map_err(|_| HatchetError::MissingEnvVar {
            var: String::from("HATCHET_CLIENT_TOKEN"),
        })?;

        Ok(Self::new(token)?)
    }

    fn decode_token(token_payload: &str) -> Result<serde_json::Value, HatchetError> {
        let payload_bytes = URL_SAFE_NO_PAD.decode(token_payload)?;
        let payload_json: serde_json::Value = serde_json::from_slice(&payload_bytes)?;
        Ok(payload_json)
    }

    fn parse_token(payload_json: serde_json::Value) -> Result<(String, String), HatchetError> {
        let grpc_address = payload_json
            .get("grpc_broadcast_address")
            .ok_or(HatchetError::MissingTokenField("grpc_broadcast_address"))?;

        let server_url = payload_json
            .get("server_url")
            .ok_or(HatchetError::MissingTokenField("server_url"))?;

        Ok((grpc_address.to_string(), server_url.to_string()))
    }
}
