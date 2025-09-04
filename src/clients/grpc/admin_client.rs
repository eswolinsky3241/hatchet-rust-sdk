use super::v1::workflows::CreateWorkflowVersionRequest;
use super::v1::workflows::admin_service_client::AdminServiceClient;
use crate::HatchetError;

#[derive(Clone, Debug)]
pub(crate) struct AdminClient {
    client: AdminServiceClient<tonic::transport::Channel>,
    api_token: String,
}

impl AdminClient {
    pub(crate) fn new(channel: tonic::transport::Channel, api_token: String) -> Self {
        let client = AdminServiceClient::new(channel);
        Self { client, api_token }
    }
}

impl AdminClient {
    pub async fn put_workflow(
        &mut self,
        workflow: CreateWorkflowVersionRequest,
    ) -> Result<(), HatchetError> {
        let mut request = tonic::Request::new(workflow);
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        self.client.put_workflow(request).await?;
        Ok(())
    }
}
