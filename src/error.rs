use thiserror::Error;

#[derive(Clone, Debug, Error)]
pub enum HatchetError {
    #[error("missing required environment variable \"{0}\"")]
    MissingEnvVar(String),
    #[error("token should have three parts")]
    InvalidTokenFormat,
    #[error("{0}")]
    Base64DecodeError(#[from] base64::DecodeError),
    #[error("Error decoding JSON: {0}")]
    JsonDecodeError(String),
    #[error("Error encoding JSON: {0}")]
    JsonEncode(String),
    #[error("Missing required field in JWT payload: {0}")]
    MissingTokenField(&'static str),
    #[error("Invalid authorization header in gRPC request: {0}")]
    InvalidAuthHeader(String),
    #[error("Unable to connect to gRPC server: {0}")]
    GrpcConnect(String),
    #[error("Response missing output")]
    MissingOutput,
    #[error("{0}")]
    WorkflowFailed(String),
    #[error("invalid gRPC URI: {0}")]
    InvalidUri(String),
    #[error("no tasks found in workflow")]
    MissingTasks,
    #[error("{0}")]
    SystemTimeError(#[from] std::time::SystemTimeError),
    #[error("unrecognized action received: {0}")]
    UnrecognizedAction(String),
    #[error("task not found: {task_name}")]
    TaskNotFound { task_name: String },
    #[error("Parent task not found: {parent_step_name}")]
    ParentTaskNotFound { parent_step_name: String },
    #[error("Invalid gRPC address: {0}")]
    InvalidGrpcAddress(String),
    #[error("Invalid TLS strategy: '{0}'. Valid options are 'none' or 'tls'")]
    InvalidTlsStrategy(String),
    #[error("gRPC request returned error with status {0}")]
    GrpcErrorStatus(#[from] tonic::Status),
    #[error("Error installing default crypto provider")]
    CryptoProvider,
    #[error("{0}")]
    RestApiError(String),
    #[error("Error sending message to dispatcher: {0}")]
    DispatchError(String),
}
