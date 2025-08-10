fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure().compile_protos(
        &[
            "api-contracts/protos/dispatcher/dispatcher.proto",
            "api-contracts/protos/events/events.proto",
            "api-contracts/protos/v1/dispatcher.proto",
            "api-contracts/protos/v1/workflows.proto",
            "api-contracts/protos/workflows/workflows.proto",
        ],
        &["api-contracts/protos"],
    )?;
    Ok(())
}
