pub(crate) mod v1 {
    pub(crate) mod dispatcher {
        tonic::include_proto!("v1");
    }

    pub(crate) mod workflows {
        tonic::include_proto!("v1");
    }
}

pub(crate) mod v0 {
    pub(crate) mod dispatcher {
        tonic::include_proto!("_");
    }

    pub(crate) mod events {
        tonic::include_proto!("_");
    }

    pub(crate) mod workflows {
        tonic::include_proto!("_");
    }
}

pub mod admin_client;
pub mod dispatcher_client;
pub mod event_client;
pub mod workflow_client;

pub(crate) use admin_client::AdminClient;
pub(crate) use dispatcher_client::DispatcherClient;
pub(crate) use event_client::EventClient;
pub(crate) use workflow_client::WorkflowClient;
