# AGENTS.md

## Repository purpose

- Unofficial Rust SDK for Hatchet.
- Single crate published as `hatchet-sdk`.
- Primary user-facing API centers on `Hatchet`, `Task`, `Workflow`, and `Worker`.

## Project layout

- `src/clients/hatchet.rs`: main client entry point.
- `src/runnables/`: task/workflow abstractions and runnable trait.
- `src/worker/`: worker runtime, listener, and dispatching.
- `src/context.rs`: execution context passed to handlers.
- `src/config.rs`: environment/token configuration parsing.
- `src/clients/grpc/`: gRPC clients and generated proto bindings.
- `src/clients/rest/`: REST client and higher-level features.
- `api-contracts/protos/`: protobuf contracts used at build time.
- `tests/integration_tests.rs`: integration tests with testcontainers.

## Build and test

- Build: `cargo build`
- Format check: `cargo fmt --check`
- Unit tests: `cargo test --lib`
- Doc tests: `cargo test --doc`
- Integration tests: `cargo test --test integration_tests`

## Environment requirements

- `protoc` must be installed and available on `PATH`.
- Docker is required for integration tests.
- Common runtime env vars:
  - `HATCHET_CLIENT_TOKEN`
  - `HATCHET_CLIENT_TLS_STRATEGY`

## Implementation guidance

- Keep changes minimal and focused to the requested task.
- Prefer edits in handwritten code under `src/` over modifying generated REST models/apis unless regeneration is intended.
- Preserve public API compatibility unless a breaking change is explicitly requested.
- If changing behavior in worker dispatch, task execution, or context handling, add or update tests.
- Use existing builder patterns and error handling conventions already present in the crate.

## Validation expectations

- For code changes, run at least `cargo fmt --check` and relevant tests for touched areas.
- Run integration tests only when changes affect cross-service behavior and Docker is available.
