use std::sync::Arc;

use super::pagination::{PaginationResponse, hash_map_to_value};
use crate::clients::rest::apis::workflow_api::{
    cron_workflow_list, workflow_cron_delete, workflow_cron_get,
};
use crate::clients::rest::apis::workflow_run_api::cron_workflow_trigger_create;
use crate::clients::rest::models::{
    CronWorkflowList200Response, CronWorkflowTriggerCreate200Response,
    CronWorkflowTriggerCreateRequest,
};
use crate::{Configuration, HatchetError};

/// Client for managing cron workflow triggers. Accessed via [`Hatchet::crons`](crate::Hatchet).
///
/// Supports both 5-field (`* * * * *`) and 6-field (`* * * * * *`) cron expressions.
/// Requires a token with a `sub` (tenant ID) claim.
#[derive(Clone, Debug)]
pub struct CronsClient {
    configuration: Arc<Configuration>,
    tenant_id: Option<String>,
}

impl CronsClient {
    pub(crate) fn new(configuration: Arc<Configuration>, tenant_id: Option<String>) -> Self {
        Self {
            configuration,
            tenant_id,
        }
    }

    fn tenant_id(&self) -> Result<&str, HatchetError> {
        self.tenant_id
            .as_deref()
            .ok_or_else(|| HatchetError::MissingTokenField("sub"))
    }

    /// Create a cron trigger for the given workflow. Validates the expression client-side before calling the API.
    pub async fn create(
        &self,
        workflow_name: &str,
        opts: CreateCronOpts,
    ) -> Result<CronTrigger, HatchetError> {
        // cron crate requires seconds field; prepend "0" for 5-field expressions
        let normalized = if opts.expression.split_whitespace().count() == 5 {
            format!("0 {}", opts.expression)
        } else {
            opts.expression.clone()
        };
        normalized
            .parse::<cron::Schedule>()
            .map_err(|_| HatchetError::InvalidCronExpression(opts.expression.clone()))?;

        let request = CronWorkflowTriggerCreateRequest {
            input: opts.input,
            additional_metadata: opts
                .additional_metadata
                .unwrap_or_else(|| serde_json::json!({})),
            cron_name: opts.name,
            cron_expression: opts.expression,
            priority: opts.priority,
        };

        let tenant = self.tenant_id()?;
        cron_workflow_trigger_create(&self.configuration, tenant, workflow_name, request)
            .await
            .map(Into::into)
            .map_err(HatchetError::from_rest)
    }

    /// Retrieve a cron trigger by ID.
    pub async fn get(&self, cron_id: &str) -> Result<CronTrigger, HatchetError> {
        let tenant = self.tenant_id()?;
        workflow_cron_get(&self.configuration, tenant, cron_id)
            .await
            .map(Into::into)
            .map_err(HatchetError::from_rest)
    }

    /// List cron triggers, optionally filtered by workflow, name, or metadata.
    pub async fn list(&self, opts: ListCronsOpts) -> Result<CronTriggerList, HatchetError> {
        let tenant = self.tenant_id()?;
        cron_workflow_list(
            &self.configuration,
            tenant,
            opts.offset,
            opts.limit,
            opts.workflow_id.as_deref(),
            opts.workflow_name.as_deref(),
            opts.cron_name.as_deref(),
            opts.additional_metadata,
            opts.order_by_field.as_deref(),
            opts.order_by_direction.as_deref(),
        )
        .await
        .map(Into::into)
        .map_err(HatchetError::from_rest)
    }

    /// Delete a cron trigger by ID.
    pub async fn delete(&self, cron_id: &str) -> Result<(), HatchetError> {
        let tenant = self.tenant_id()?;
        workflow_cron_delete(&self.configuration, tenant, cron_id)
            .await
            .map_err(HatchetError::from_rest)
    }
}

/// Options for creating a cron trigger via [`CronsClient::create`].
#[derive(Clone, Debug)]
pub struct CreateCronOpts {
    pub name: String,
    pub expression: String,
    pub input: serde_json::Value,
    pub additional_metadata: Option<serde_json::Value>,
    pub priority: Option<i32>,
}

/// Filter and pagination options for [`CronsClient::list`].
#[derive(Clone, Debug, Default)]
pub struct ListCronsOpts {
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    pub workflow_id: Option<String>,
    pub workflow_name: Option<String>,
    pub cron_name: Option<String>,
    pub additional_metadata: Option<Vec<String>>,
    pub order_by_field: Option<String>,
    pub order_by_direction: Option<String>,
}

/// A cron workflow trigger returned by the Hatchet API.
#[derive(Clone, Debug)]
pub struct CronTrigger {
    pub metadata_id: String,
    pub cron: String,
    pub name: Option<String>,
    pub workflow_id: String,
    pub workflow_name: String,
    pub input: serde_json::Value,
    pub additional_metadata: serde_json::Value,
    pub enabled: bool,
    pub priority: Option<i32>,
}

impl From<CronWorkflowTriggerCreate200Response> for CronTrigger {
    fn from(r: CronWorkflowTriggerCreate200Response) -> Self {
        Self {
            metadata_id: r.metadata.id,
            cron: r.cron,
            name: r.name,
            workflow_id: r.workflow_id,
            workflow_name: r.workflow_name,
            input: hash_map_to_value(r.input),
            additional_metadata: hash_map_to_value(r.additional_metadata),
            enabled: r.enabled,
            priority: r.priority,
        }
    }
}

/// Paginated list of cron triggers returned by [`CronsClient::list`].
#[derive(Clone, Debug)]
pub struct CronTriggerList {
    pub rows: Vec<CronTrigger>,
    pub pagination: Option<PaginationResponse>,
}

impl From<CronWorkflowList200Response> for CronTriggerList {
    fn from(r: CronWorkflowList200Response) -> Self {
        Self {
            rows: r
                .rows
                .unwrap_or_default()
                .into_iter()
                .map(Into::into)
                .collect(),
            pagination: r.pagination.map(Into::into),
        }
    }
}

/// Optional parameters for [`Task::cron`](crate::Task::cron) and [`Workflow::cron`](crate::Workflow::cron).
#[derive(Clone, Debug, Default)]
pub struct CronOptions {
    pub additional_metadata: Option<serde_json::Value>,
    pub priority: Option<i32>,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_six_field_cron_expression_parses() {
        assert!("0 */2 * * * *".parse::<cron::Schedule>().is_ok());
    }

    #[test]
    fn test_five_field_cron_with_prepended_seconds_parses() {
        let five_field = "*/2 * * * *";
        let normalized = format!("0 {}", five_field);
        assert!(normalized.parse::<cron::Schedule>().is_ok());
    }

    #[test]
    fn test_invalid_cron_expression_fails() {
        assert!("not a cron".parse::<cron::Schedule>().is_err());
    }

    #[test]
    fn test_missing_tenant_returns_error() {
        let client = super::CronsClient::new(
            std::sync::Arc::new(crate::Configuration {
                base_path: String::new(),
                client: reqwest::Client::new(),
                basic_auth: None,
                oauth_access_token: None,
                bearer_access_token: None,
                user_agent: None,
                api_key: None,
            }),
            None,
        );
        let err = client.tenant_id().unwrap_err();
        assert!(matches!(err, crate::HatchetError::MissingTokenField("sub")));
    }
}
