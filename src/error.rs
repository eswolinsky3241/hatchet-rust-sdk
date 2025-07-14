use thiserror::Error;

#[derive(Debug, Error)]
pub enum HatchetError {
    #[error("Missing required environment variable \"{var}\".")]
    MissingEnvVar { var: String },
    #[error("Token should have three parts.")]
    InvalidTokenFormat,
    #[error("Error decoding token: {0}.")]
    Base64DecodeError(#[from] base64::DecodeError),
    #[error("Error decoding JSON.")]
    JsonDecodeError(#[from] serde_json::Error),
    #[error("Missing required field in JWT payload: {0}")]
    MissingTokenField(&'static str),
    #[error("Error sending API request: {0}")]
    ApiRequestError(#[from] reqwest::Error),
    #[error("Hatchet request failed:\nurl: {method} {url}\nstatus: {status}\ncontents: {body}")]
    HttpError {
        url: String,
        method: reqwest::Method,
        status: reqwest::StatusCode,
        body: String,
    },
    #[error("Error encoding JSON")]
    JsonEncode(serde_json::Error),
    #[error("Invalid authorization header in gRPC request")]
    InvalidAuthHeader(tonic::metadata::errors::InvalidMetadataValue),
    #[error("Unable to connect to gRPC server")]
    GrpcConnect(#[from] tonic::transport::Error),
    #[error("Error calling gRPC service")]
    GrpcCall(tonic::Status),
    #[error("Response missing output")]
    MissingOutput,
    #[error("Workflow failed:\n{error_message}")]
    WorkflowFailed { error_message: String },
    #[error("Invalid gRPC URI: {uri}")]
    InvalidUri { uri: String },
    #[error("No tasks found in workflow.")]
    MissingTasks,
}
