use std::future::Future;
use std::pin::Pin;

use crate::{Context, HatchetError};

/// A boxed async task function type.
pub type TaskFn<I, O> = Box<
    dyn Fn(I, Context) -> Pin<Box<dyn Future<Output = Result<O, HatchetError>> + Send>>
        + Send
        + Sync,
>;

/// A type-erased task function that can handle any input/output types.
pub type ErasedTaskFn = Box<
    dyn Fn(
            serde_json::Value,
            Context,
        ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, HatchetError>> + Send>>
        + Send
        + Sync,
>;
