use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::HatchetError;
use crate::workflows::Context;

#[async_trait::async_trait]
pub trait TaskFunction<I, O>: Send + Sync
where
    I: DeserializeOwned + Send + 'static,
    O: Serialize + Send + 'static,
{
    fn name(&self) -> &'static str;

    async fn run(&self, input: I, ctx: Context) -> Result<O, HatchetError>;
}
