use crate::clients::rest::apis::configuration;
use crate::clients::rest::apis::workflow_runs_api::v1_workflow_run_get;
use crate::clients::rest::models::V1WorkflowRunCreate200Response;
use crate::error::HatchetError;

#[derive(Clone, Debug)]
pub struct WorkflowsClient<'a> {
    configuration: &'a configuration::Configuration,
}

impl<'a> WorkflowsClient<'a> {
    pub async fn get(
        &self,
        workflow_run_id: &str,
    ) -> Result<V1WorkflowRunCreate200Response, HatchetError> {
        let response = v1_workflow_run_get(&self.configuration, workflow_run_id)
            .await
            .unwrap();
        Ok(response)
    }
}
