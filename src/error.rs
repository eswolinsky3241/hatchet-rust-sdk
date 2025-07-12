#[derive(Debug)]
pub enum HatchetError {
    MissingEnvVar(std::env::VarError),
    InvalidTokenFormat,
    Base64Decode(base64::DecodeError),
    JsonDecode(serde_json::Error),
    MissingGrpcAddress,
    MissingServerUrl,
    ApiRequestError(reqwest::Error),
    JsonEncode(serde_json::Error),
    HttpJsonDecode(reqwest::Error),
    InvalidAuthHeader(tonic::metadata::errors::InvalidMetadataValue),
    GrpcConnect(tonic::transport::Error),
    GrpcCall(tonic::Status),
}
