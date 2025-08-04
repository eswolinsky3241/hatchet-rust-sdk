use std::future::Future;
use std::pin::Pin;

use crate::{Context, HatchetError};

pub(crate) type TaskFn<I, O> = Box<
    dyn Fn(I, Context) -> Pin<Box<dyn Future<Output = Result<O, HatchetError>> + Send>>
        + Send
        + Sync,
>;

pub(crate) type ErasedTaskFn = Box<
    dyn Fn(
            serde_json::Value,
            Context,
        ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, HatchetError>> + Send>>
        + Send
        + Sync,
>;
