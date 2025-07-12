fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::compile_protos("api-contracts/protos/workflows.proto")?;
    Ok(())
}
