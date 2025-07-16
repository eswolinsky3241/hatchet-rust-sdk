fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure().compile_protos(
        &[
            "api-contracts/protos/workflows.proto",
            "api-contracts/protos/dispatcher.proto",
        ],
        &["api-contracts/protos"],
    )?;
    Ok(())
}
