# AGENTS.md

Unofficial Rust SDK for Hatchet (`hatchet-sdk` crate).

## Key files

- `src/clients/hatchet.rs`: main client API (`Hatchet`)
- `src/runnables/`: `Task`, `Workflow`, `Runnable`
- `src/worker/`: worker runtime and dispatch
- `src/context.rs`, `src/config.rs`: handler context and env/token config
- `src/clients/grpc/`, `api-contracts/protos/`: gRPC clients and proto contracts
- `src/clients/rest/`: REST client code

## Build and test

- `cargo build`
- `cargo fmt --check`
- `cargo test --lib`
- `cargo test --doc`
- `cargo test --test integration_tests`

## Requirements

- `protoc` on `PATH`
- Docker for integration tests
- Common env vars: `HATCHET_CLIENT_TOKEN`, `HATCHET_CLIENT_TLS_STRATEGY`

## Working rules

- Keep changes minimal and task-focused.
- Prefer handwritten code changes over editing generated REST code unless regenerating.
- Keep public API compatibility unless a breaking change is requested.
- Add/update tests when changing worker dispatch, task execution, or context behavior.
- Run `cargo fmt --check` and relevant tests for touched areas.
