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
