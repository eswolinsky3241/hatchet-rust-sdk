use std::marker::PhantomData;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::HatchetError;
use crate::tasks::task_trait::Task;

#[async_trait::async_trait]
pub trait ErasedTask: Send + Sync {
    fn name(&self) -> &'static str;

    async fn run_from_json(
        &self,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, HatchetError>;
}

pub struct ErasedTaskImpl<T, I, O>
where
    T: Task<I, O>,
    I: DeserializeOwned + Send + 'static,
    O: Serialize + Send + 'static,
{
    inner: T,
    _marker: PhantomData<fn(I) -> O>,
}

impl<T, I, O> ErasedTaskImpl<T, I, O>
where
    T: Task<I, O>,
    I: DeserializeOwned + Send + 'static,
    O: Serialize + Send + 'static,
{
    pub fn new(inner: T) -> Self {
        Self {
            inner,
            _marker: PhantomData,
        }
    }
}

#[async_trait::async_trait]
impl<T, I, O> ErasedTask for ErasedTaskImpl<T, I, O>
where
    T: Task<I, O>,
    I: DeserializeOwned + Send + 'static,
    O: Serialize + Send + 'static,
{
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    async fn run_from_json(
        &self,
        input: serde_json::Value,
    ) -> Result<serde_json::Value, HatchetError> {
        let typed_input: I = serde_json::from_value(input)?;
        let output = self.inner.run(typed_input).await?;
        Ok(serde_json::to_value(output)?)
    }
}
