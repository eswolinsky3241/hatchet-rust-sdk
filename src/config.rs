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
            .and_then(|v| v.as_str())
            .ok_or(HatchetError::MissingTokenField("grpc_broadcast_address"))?;

        let server_url = payload_json
            .get("server_url")
            .and_then(|v| v.as_str())
            .ok_or(HatchetError::MissingTokenField("server_url"))?;

        Ok((grpc_address.to_string(), server_url.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_token_without_three_parts_raises_error() {
        let config = HatchetConfig::new(String::from("part0.part1.part2.part3"));
        assert!(matches!(config, Err(HatchetError::InvalidTokenFormat)));
        let config = HatchetConfig::new(String::from("part0.part1"));
        assert!(matches!(config, Err(HatchetError::InvalidTokenFormat)));
    }

    #[test]
    fn test_token_decoded_into_config() {
        let config = HatchetConfig::new(format!("header.{}.sig", "eyJzZXJ2ZXJfdXJsIjoiaHR0cHM6Ly9oYXRjaGV0LmNvbSIsImdycGNfYnJvYWRjYXN0X2FkZHJlc3MiOiJlbmdpbmUuaGF0Y2hldC5jb20ifQ".to_string())).unwrap();
        assert_eq!(config.server_url, "https://hatchet.com");
        assert_eq!(config.grpc_address, "engine.hatchet.com");
    }
}
