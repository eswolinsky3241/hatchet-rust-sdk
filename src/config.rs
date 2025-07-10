use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use std::env;

#[derive(Debug)]
pub struct HatchetConfig {
    pub api_token: String,
    pub grpc_address: String,
    pub server_url: String,
}

impl HatchetConfig {
    pub fn from_env() -> Result<Self, HatchetConfigError> {
        let token = env::var("HATCHET_CLIENT_TOKEN").map_err(HatchetConfigError::MissingEnvVar)?;
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() < 2 {
            return Err(HatchetConfigError::InvalidTokenFormat);
        }

        let payload_bytes = URL_SAFE_NO_PAD
            .decode(parts[1])
            .map_err(HatchetConfigError::Base64Decode)?;
        let payload_json: serde_json::Value =
            serde_json::from_slice(&payload_bytes).map_err(HatchetConfigError::JsonDecode)?;

        let grpc_address_no_scheme = payload_json
            .get("grpc_broadcast_address")
            .and_then(|v| v.as_str())
            .ok_or(HatchetConfigError::MissingGrpcAddress)?;

        let grpc_address = "http://".to_owned() + grpc_address_no_scheme;

        let server_url = payload_json
            .get("server_url")
            .and_then(|v| v.as_str())
            .ok_or(HatchetConfigError::MissingServerUrl)?;

        Ok(Self {
            api_token: token,
            grpc_address: grpc_address.to_string(),
            server_url: server_url.to_string(),
        })
    }
}

#[derive(Debug)]
pub enum HatchetConfigError {
    MissingEnvVar(std::env::VarError),
    InvalidTokenFormat,
    Base64Decode(base64::DecodeError),
    JsonDecode(serde_json::Error),
    MissingGrpcAddress,
    MissingServerUrl,
}
