use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SimpleInput {
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SimpleOutput {
    pub transformed_message: String,
}
