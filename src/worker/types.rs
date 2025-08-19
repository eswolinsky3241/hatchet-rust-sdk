use std::future::Future;
use std::pin::Pin;

use crate::HatchetError;

pub(crate) type TaskFn<I, O, X> = Box<
    dyn Fn(I, X) -> Pin<Box<dyn Future<Output = Result<O, HatchetError>> + Send>> + Send + Sync,
>;

pub(crate) type ErasedTaskFn<X> = Box<
    dyn Fn(
            serde_json::Value,
            X,
        ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, HatchetError>> + Send>>
        + Send
        + Sync,
>;
