use std::cell::RefCell;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use prost_types::Timestamp;
use tonic::Request;
use tonic::metadata::MetadataValue;

use crate::HatchetError;

pub(crate) fn proto_timestamp_now() -> Result<Timestamp, HatchetError> {
    let now = SystemTime::now().duration_since(UNIX_EPOCH)?;

    Ok(Timestamp {
        seconds: now.as_secs() as i64,
        nanos: now.subsec_nanos() as i32,
    })
}

pub(crate) fn add_grpc_auth_header<T>(
    request: &mut Request<T>,
    token: &str,
) -> Result<(), HatchetError> {
    let token_header: MetadataValue<_> = format!("Bearer {}", token).parse().map_err(
        |e: tonic::metadata::errors::InvalidMetadataValue| {
            HatchetError::InvalidAuthHeader(e.to_string())
        },
    )?;
    request.metadata_mut().insert("authorization", token_header);
    Ok(())
}

/// Converts a std::time::Duration to a string expression.
pub(crate) fn duration_to_expr(duration: Duration) -> String {
    const HOUR: u64 = 3600;
    const MINUTE: u64 = 60;

    let seconds = duration.as_secs();

    if seconds == 0 {
        return String::from("0s");
    }
    if seconds % HOUR == 0 {
        return format!("{}h", seconds / HOUR);
    }
    if seconds % MINUTE == 0 {
        return format!("{}m", seconds / MINUTE);
    }
    format!("{}s", seconds)
}

#[derive(Clone, Debug)]
pub(crate) struct ExecutionContext {
    pub(crate) workflow_run_id: String,
    pub(crate) step_run_id: String,
    pub(crate) child_index: i32,
}

tokio::task_local! {
    pub(crate) static EXECUTION_CONTEXT: RefCell<ExecutionContext>;
}

/// A type that serializes to an empty JSON object.
/// This can be used for workflows that don't need input or don't return output.
///
/// # Example
/// ```rust
/// use hatchet_sdk::EmptyModel;
///
/// // EmptyModel serializes to an empty JSON object
/// let empty_input = EmptyModel;
/// let serialized = serde_json::to_value(empty_input).unwrap();
/// assert_eq!(serialized, serde_json::json!({}));
/// ```
#[derive(Debug, Clone, Copy)]
pub struct EmptyModel;

impl serde::Serialize for EmptyModel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        use serde::ser::SerializeMap;
        let map = serializer.serialize_map(Some(0))?;
        map.end()
    }
}

impl<'de> serde::Deserialize<'de> for EmptyModel {
    fn deserialize<D>(deserializer: D) -> Result<EmptyModel, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use std::fmt;

        use serde::de::{self, MapAccess, Visitor};

        struct EmptyModelVisitor;

        impl<'de> Visitor<'de> for EmptyModelVisitor {
            type Value = EmptyModel;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an empty object")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                if map.next_entry::<String, serde_json::Value>()?.is_some() {
                    return Err(de::Error::custom("expected empty object"));
                }
                Ok(EmptyModel)
            }
        }

        deserializer.deserialize_map(EmptyModelVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input_serialization() {
        let empty_model = EmptyModel;
        let serialized = serde_json::to_value(empty_model).unwrap();
        assert_eq!(serialized, serde_json::json!({}));
    }

    #[test]
    fn test_empty_input_deserialization() {
        let json = serde_json::json!({});
        let empty_model: EmptyModel = serde_json::from_value(json).unwrap();
        assert_eq!(format!("{:?}", empty_model), "EmptyModel");
    }

    #[test]
    fn test_duration_to_expr_hours() {
        assert_eq!(duration_to_expr(Duration::from_secs(3600)), "1h");
        assert_eq!(duration_to_expr(Duration::from_secs(7200)), "2h");
        assert_eq!(duration_to_expr(Duration::from_secs(18000)), "5h");
    }

    #[test]
    fn test_duration_to_expr_minutes() {
        assert_eq!(duration_to_expr(Duration::from_secs(60)), "1m");
        assert_eq!(duration_to_expr(Duration::from_secs(120)), "2m");
        assert_eq!(duration_to_expr(Duration::from_secs(300)), "5m");
        assert_eq!(duration_to_expr(Duration::from_secs(3540)), "59m");
    }

    #[test]
    fn test_duration_to_expr_seconds() {
        assert_eq!(duration_to_expr(Duration::from_secs(1)), "1s");
        assert_eq!(duration_to_expr(Duration::from_secs(30)), "30s");
        assert_eq!(duration_to_expr(Duration::from_secs(45)), "45s");
        assert_eq!(duration_to_expr(Duration::from_secs(59)), "59s");
        assert_eq!(duration_to_expr(Duration::from_secs(3661)), "3661s");
    }

    #[test]
    fn test_duration_to_expr_zero() {
        assert_eq!(duration_to_expr(Duration::from_secs(0)), "0s");
    }
}
