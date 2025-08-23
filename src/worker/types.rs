use std::future::Future;
use std::pin::Pin;

use crate::workflows::context::Context;

pub(crate) type HatchetTaskResult<O, E> = Result<O, E>;
pub(crate) type HatchetTaskFuture<O, E> =
    Pin<Box<dyn Future<Output = HatchetTaskResult<O, E>> + Send>>;

pub(crate) type HatchetTaskFunction<I, O, E> =
    Box<dyn Fn(I, Context) -> HatchetTaskFuture<O, E> + Send + Sync>;

pub(crate) type ErasedHatchetTaskResult = Pin<
    Box<dyn Future<Output = Result<serde_json::Value, Box<dyn std::error::Error + Send>>> + Send>,
>;

pub(crate) type ErasedHatchetTaskFunction =
    Box<dyn Fn(serde_json::Value, Context) -> ErasedHatchetTaskResult + Send + Sync>;
