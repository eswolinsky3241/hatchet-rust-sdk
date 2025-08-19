use std::future::Future;
use std::pin::Pin;

use crate::HatchetError;
use crate::workflows::context::HatchetContextTrait;

pub(crate) type TaskFn<I, O, X>
where
    X: HatchetContextTrait + Send + Sync + 'static,
= Box<dyn Fn(I, X) -> Pin<Box<dyn Future<Output = Result<O, HatchetError>> + Send>> + Send + Sync>;

pub(crate) type ErasedTaskFn<X>
where
    X: HatchetContextTrait,
= Box<
    dyn Fn(
            serde_json::Value,
            X,
        ) -> Pin<Box<dyn Future<Output = Result<serde_json::Value, HatchetError>> + Send>>
        + Send
        + Sync,
>;
