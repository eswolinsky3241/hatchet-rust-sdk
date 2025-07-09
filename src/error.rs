#[derive(Debug)]
pub enum HatchetError {
    // Existing from config
    MissingEnvVar(std::env::VarError),
    InvalidTokenFormat,
    Base64Decode(base64::DecodeError),
    JsonDecode(serde_json::Error),
    MissingGrpcAddress,
    MissingServerUrl,

    // New for client
    JsonEncode(serde_json::Error),
    InvalidAuthHeader(tonic::metadata::errors::InvalidMetadataValue),
    GrpcConnect(tonic::transport::Error),
    GrpcCall(tonic::Status),
}
