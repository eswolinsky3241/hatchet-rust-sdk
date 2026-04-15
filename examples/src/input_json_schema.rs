use hatchet_sdk::{Context, Hatchet, Runnable, anyhow, serde_json, tokio};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct SchemaInput {
    pub first: i64,
    pub second: i64,
}

#[derive(Serialize, Deserialize)]
pub struct AddOutput {
    pub value: i64,
}

pub async fn create_schema_workflow() -> hatchet_sdk::Workflow<SchemaInput, AddOutput> {
    let hatchet = Hatchet::from_env().await.unwrap();

    let schema = schemars::schema_for!(SchemaInput);
    let schema_value = serde_json::to_value(schema).unwrap();

    let task = hatchet
        .task(
            "add",
            async move |input: SchemaInput, _context: Context| -> anyhow::Result<AddOutput> {
                Ok(AddOutput {
                    value: input.first + input.second,
                })
            },
        )
        .build()
        .unwrap();

    hatchet
        .workflow::<SchemaInput, AddOutput>("schema-workflow")
        .input_json_schema(Some(schema_value))
        .build()
        .unwrap()
        .add_task(&task)
}

#[tokio::main]
#[allow(dead_code)]
async fn main() {
    dotenvy::dotenv().ok();

    let workflow = create_schema_workflow().await;

    let input = SchemaInput {
        first: 3,
        second: 7,
    };
    let result = workflow.run(&input, None).await.unwrap();
    println!("Result: {}", result.value);
}
