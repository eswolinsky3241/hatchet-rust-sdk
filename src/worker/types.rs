use std::future::Future;
use std::pin::Pin;

use crate::HatchetError;
use crate::workflows::context::Context;

pub(crate) type TaskFn<I, O, C> = Box<
    dyn Fn(I, Context<C>) -> Pin<Box<dyn Future<Output = Result<O, HatchetError>> + Send>>
        + Send
        + Sync,
>;

pub(crate) type ErasedTaskFn<C> = Box<
    dyn Fn(
            serde_json::Value,
            Context<C>,
        ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, HatchetError>> + Send>>
        + Send
        + Sync,
>;
