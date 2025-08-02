use crate::HatchetError;
use crate::workflows::task_function::TaskFunction;

pub struct Task<T> {
    pub name: String,
    pub function: T,
    pub parents: Vec<String>,
}

impl<T> Task<T> {
    pub fn new(name: String, function: T) -> Self {
        Self {
            name,
            function,
            parents: vec![],
        }
    }

    pub fn add_parent<U>(&mut self, parent: Self) -> () {
        self.parents.push(parent.name);
    }

    pub(crate) async fn run<I, O>(&self, input: I, ctx: crate::Context) -> Result<O, HatchetError>
    where
        I: serde::de::DeserializeOwned + Send + Sync + 'static,
        O: serde::Serialize + Send + Sync + 'static,
        T: TaskFunction<I, O>,
    {
        self.function.run(input, ctx).await
    }
}
