use std::future::Future;
use std::pin::Pin;

use crate::workflows::context::Context;

pub(crate) type HatchetTaskResult<O> = Result<O, Box<dyn std::error::Error + Send>>;
pub(crate) type HatchetTaskFuture<O> = Pin<Box<dyn Future<Output = HatchetTaskResult<O>> + Send>>;

pub(crate) type TaskFn<I, O> = Box<dyn Fn(I, Context) -> HatchetTaskFuture<O> + Send + Sync>;

pub(crate) type ErasedHatchetTaskResult = Pin<
    Box<dyn Future<Output = Result<serde_json::Value, Box<dyn std::error::Error + Send>>> + Send>,
>;

pub(crate) type ErasedTaskFn =
    Box<dyn Fn(serde_json::Value, Context) -> ErasedHatchetTaskResult + Send + Sync>;
