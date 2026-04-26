use std::sync::Arc;

use chrono::{DateTime, Utc};

use super::pagination::{PaginationResponse, hash_map_to_value};
use crate::clients::rest::apis::workflow_api::{
    workflow_scheduled_delete, workflow_scheduled_get, workflow_scheduled_list,
};
use crate::clients::rest::apis::workflow_run_api::scheduled_workflow_run_create;
use crate::clients::rest::models::{
    ScheduledWorkflowRunCreate200Response, ScheduledWorkflowRunCreateRequest,
    WorkflowScheduledList200Response,
};
use crate::{Configuration, HatchetError};

/// Client for managing scheduled (one-time) workflow runs. Accessed via [`Hatchet::schedules`](crate::Hatchet).
///
/// Requires a token with a `sub` (tenant ID) claim.
#[derive(Clone, Debug)]
pub struct SchedulesClient {
    configuration: Arc<Configuration>,
    tenant_id: Option<String>,
}

impl SchedulesClient {
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

    /// Schedule a workflow to run at a specific future time.
    pub async fn create(
        &self,
        workflow_name: &str,
        opts: CreateScheduleOpts,
    ) -> Result<ScheduledRun, HatchetError> {
        let tenant = self.tenant_id()?;
        let request = ScheduledWorkflowRunCreateRequest {
            input: opts.input,
            additional_metadata: opts
                .additional_metadata
                .unwrap_or_else(|| serde_json::json!({})),
            trigger_at: opts.trigger_at.to_rfc3339(),
            priority: opts.priority,
        };

        scheduled_workflow_run_create(&self.configuration, tenant, workflow_name, request)
            .await
            .map(Into::into)
            .map_err(HatchetError::from_rest)
    }

    /// Retrieve a scheduled run by ID.
    pub async fn get(&self, scheduled_id: &str) -> Result<ScheduledRun, HatchetError> {
        let tenant = self.tenant_id()?;
        workflow_scheduled_get(&self.configuration, tenant, scheduled_id)
            .await
            .map(Into::into)
            .map_err(HatchetError::from_rest)
    }

    /// List scheduled runs, optionally filtered by workflow, status, or metadata.
    pub async fn list(&self, opts: ListSchedulesOpts) -> Result<ScheduledRunList, HatchetError> {
        let tenant = self.tenant_id()?;
        workflow_scheduled_list(
            &self.configuration,
            tenant,
            opts.offset,
            opts.limit,
            opts.order_by_field.as_deref(),
            opts.order_by_direction.as_deref(),
            opts.workflow_id.as_deref(),
            opts.parent_workflow_run_id.as_deref(),
            opts.parent_task_run_external_id.as_deref(),
            opts.additional_metadata,
            opts.statuses,
        )
        .await
        .map(Into::into)
        .map_err(HatchetError::from_rest)
    }

    /// Delete a scheduled run by ID.
    pub async fn delete(&self, scheduled_id: &str) -> Result<(), HatchetError> {
        let tenant = self.tenant_id()?;
        workflow_scheduled_delete(&self.configuration, tenant, scheduled_id)
            .await
            .map_err(HatchetError::from_rest)
    }
}

/// Options for creating a scheduled run via [`SchedulesClient::create`].
#[derive(Clone, Debug)]
pub struct CreateScheduleOpts {
    pub trigger_at: DateTime<Utc>,
    pub input: serde_json::Value,
    pub additional_metadata: Option<serde_json::Value>,
    pub priority: Option<i32>,
}

/// Filter and pagination options for [`SchedulesClient::list`].
#[derive(Clone, Debug, Default)]
pub struct ListSchedulesOpts {
    pub offset: Option<i64>,
    pub limit: Option<i64>,
    pub workflow_id: Option<String>,
    pub parent_workflow_run_id: Option<String>,
    pub parent_task_run_external_id: Option<String>,
    pub additional_metadata: Option<Vec<String>>,
    pub statuses: Option<Vec<String>>,
    pub order_by_field: Option<String>,
    pub order_by_direction: Option<String>,
}

/// A scheduled workflow run returned by the Hatchet API. The run will be enqueued at `trigger_at`.
#[derive(Clone, Debug)]
pub struct ScheduledRun {
    pub metadata_id: String,
    pub trigger_at: String,
    pub workflow_id: String,
    pub workflow_name: String,
    pub workflow_version_id: String,
    pub input: serde_json::Value,
    pub additional_metadata: serde_json::Value,
    pub workflow_run_id: Option<String>,
    pub workflow_run_status: Option<String>,
    pub priority: Option<i32>,
}

impl From<ScheduledWorkflowRunCreate200Response> for ScheduledRun {
    fn from(r: ScheduledWorkflowRunCreate200Response) -> Self {
        Self {
            metadata_id: r.metadata.id,
            trigger_at: r.trigger_at,
            workflow_id: r.workflow_id,
            workflow_name: r.workflow_name,
            workflow_version_id: r.workflow_version_id,
            input: hash_map_to_value(r.input),
            additional_metadata: hash_map_to_value(r.additional_metadata),
            workflow_run_id: r.workflow_run_id.map(|id| id.to_string()),
            workflow_run_status: r.workflow_run_status.and_then(|status| {
                serde_json::to_value(status)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
            }),
            priority: r.priority,
        }
    }
}

/// Paginated list of scheduled runs returned by [`SchedulesClient::list`].
#[derive(Clone, Debug)]
pub struct ScheduledRunList {
    pub rows: Vec<ScheduledRun>,
    pub pagination: Option<PaginationResponse>,
}

impl From<WorkflowScheduledList200Response> for ScheduledRunList {
    fn from(r: WorkflowScheduledList200Response) -> Self {
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

/// Optional parameters for [`Task::schedule`](crate::Task::schedule) and [`Workflow::schedule`](crate::Workflow::schedule).
#[derive(Clone, Debug, Default)]
pub struct ScheduleOptions {
    pub additional_metadata: Option<serde_json::Value>,
    pub priority: Option<i32>,
}
