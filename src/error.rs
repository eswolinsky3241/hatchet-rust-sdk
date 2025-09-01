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
    InvalidAuthHeader(#[from] tonic::metadata::errors::InvalidMetadataValue),
    #[error("Unable to connect to gRPC server")]
    GrpcConnect(#[from] tonic::transport::Error),
    #[error("Error calling gRPC service: {0}")]
    GrpcCall(tonic::Status),
    #[error("Response missing output")]
    MissingOutput,
    #[error("Workflow failed:\n{error_message}")]
    WorkflowFailed { error_message: String },
    #[error("Invalid gRPC URI: {0}")]
    InvalidUri(String),
    #[error("No tasks found in workflow.")]
    MissingTasks,
    #[error("{0}")]
    SystemTimeError(#[from] std::time::SystemTimeError),
    #[error("Unrecognized action received: {action}")]
    UnrecognizedAction { action: String },
    #[error("Task not found: {task_name}")]
    TaskNotFound { task_name: String },
    #[error("Parent task not found: {parent_step_name}")]
    ParentTaskNotFound { parent_step_name: String },
    #[error("Duplicate task name '{task_name}' in workflow '{workflow_name}'")]
    DuplicateTask {
        task_name: String,
        workflow_name: String,
    },
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
