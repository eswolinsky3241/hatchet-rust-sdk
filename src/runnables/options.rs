#[derive(derive_builder::Builder, Debug, Default, Clone)]
pub struct TriggerWorkflowOptions {
    #[builder(default = None)]
    pub additional_metadata: Option<serde_json::Value>,
    #[builder(default = None)]
    pub desired_worker_id: Option<String>,
    #[builder(default = None)]
    pub namespace: Option<String>,
    #[builder(default = false)]
    pub sticky: bool,
    #[builder(default = None)]
    pub key: Option<String>,
}
