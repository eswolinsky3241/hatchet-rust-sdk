use std::cell::RefCell;
use std::time::{SystemTime, UNIX_EPOCH};

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

pub(crate) fn add_auth_header<T>(
    request: &mut Request<T>,
    token: &str,
) -> Result<(), HatchetError> {
    let token_header: MetadataValue<_> = format!("Bearer {}", token).parse()?;
    request.metadata_mut().insert("authorization", token_header);
    Ok(())
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
/// This can be used for workflows that don't need input.
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
}
