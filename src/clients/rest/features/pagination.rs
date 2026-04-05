use std::collections::HashMap;

use crate::clients::rest::models::V1TaskEventList200ResponsePagination;

/// Converts an auto-generated `Option<HashMap<String, Value>>` to a flat `Value::Object`,
/// defaulting to `{}`. Used by feature client `From` impls.
pub(super) fn hash_map_to_value(
    value: Option<HashMap<String, serde_json::Value>>,
) -> serde_json::Value {
    value
        .and_then(|map| serde_json::to_value(map).ok())
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Pagination metadata returned by list endpoints.
#[derive(Clone, Debug)]
pub struct PaginationResponse {
    pub current_page: Option<i64>,
    pub num_pages: Option<i64>,
    pub next_page: Option<i64>,
}

impl From<Box<V1TaskEventList200ResponsePagination>> for PaginationResponse {
    fn from(response: Box<V1TaskEventList200ResponsePagination>) -> Self {
        Self {
            current_page: response.current_page,
            num_pages: response.num_pages,
            next_page: response.next_page,
        }
    }
}
