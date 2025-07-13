use thiserror::Error;

#[derive(Debug, Error)]
pub enum HatchetError {
    #[error("Missing required environment variable \"{var}\"")]
    MissingEnvVar {
        var: String,
        #[source]
        source: std::env::VarError,
    },
    #[error("Invalid token format")]
    InvalidTokenFormat,
    #[error("Error decoding JWT token")]
    Base64Decode(base64::DecodeError),
    #[error("Error decoding JSON")]
    JsonDecode(serde_json::Error),
    #[error("Missing GRPC Address")]
    MissingGrpcAddress,
    #[error("Missing API server URL")]
    MissingServerUrl,
    #[error("Error sending API request: {0}")]
    ApiRequestError(#[from] reqwest::Error),
    #[error("Error encoding JSON")]
    JsonEncode(serde_json::Error),
    #[error("HTTP {status} returned invalid JSON: {source}\nBody:\n{body}")]
    HttpJsonDecode {
        status: reqwest::StatusCode,
        body: String,
        source: serde_json::Error,
    },
    #[error("Invalid authorization header in gRPC request")]
    InvalidAuthHeader(tonic::metadata::errors::InvalidMetadataValue),
    #[error("Unable to connect to gRPC server")]
    GrpcConnect(tonic::transport::Error),
    #[error("Error calling gRPC service")]
    GrpcCall(tonic::Status),
    #[error("Response missing output")]
    MissingOutput,
    #[error("Workflow failed:\n{0}")]
    WorkflowFailed(String),
    #[error("Workflow {0} not found")]
    WorkflowNotFound(crate::workflow::RunId),
    #[error("Tasks not found")]
    NoTasks,
    #[error("Workflow has been cancelled")]
    WorkflowCancelled,
    #[error("Status {0} not recognized")]
    UnknownStatus(String),
    #[error("Error in TLS configuration")]
    TlsConfig,
}
