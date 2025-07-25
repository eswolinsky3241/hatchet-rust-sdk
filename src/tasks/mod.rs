pub(crate) mod erased;
pub mod task_trait;

pub use erased::{ErasedTask, ErasedTaskImpl};
pub use task_trait::Task;
