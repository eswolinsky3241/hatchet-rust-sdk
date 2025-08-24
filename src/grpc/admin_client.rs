use std::fmt::Debug;

use dyn_clone::DynClone;

use crate::error::HatchetError;
use crate::grpc::v1::workflows::CreateWorkflowVersionRequest;
use crate::grpc::v1::workflows::admin_service_client::AdminServiceClient;

#[async_trait::async_trait]
pub trait AdminClientTrait: Debug + Send + Sync + DynClone + 'static {
    async fn put_workflow(
        &mut self,
        workflow: CreateWorkflowVersionRequest,
    ) -> Result<(), HatchetError>;
}

dyn_clone::clone_trait_object!(AdminClientTrait);

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

#[async_trait::async_trait]
impl AdminClientTrait for AdminClient {
    async fn put_workflow(
        &mut self,
        workflow: CreateWorkflowVersionRequest,
    ) -> Result<(), HatchetError> {
        let mut request = tonic::Request::new(workflow);
        crate::utils::add_grpc_auth_header(&mut request, &self.api_token)?;
        self.client.put_workflow(request).await?;
        Ok(())
    }
}
