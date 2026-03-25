# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Unofficial Rust SDK for [Hatchet](https://hatchet.run), a distributed, fault-tolerant task queue. Single-crate library (`hatchet-sdk`) published to crates.io. Rust edition 2024.

Hatchet docs: https://docs.hatchet.run/llms.txt

## Build Requirements

`protoc` (Protocol Buffer Compiler) must be installed. The build script (`build.rs`) compiles gRPC stubs from proto files in `api-contracts/protos/`.

## Commands

```bash
cargo build                          # Build (requires protoc)
cargo fmt --check                    # Check formatting
cargo test --lib                     # Unit tests only
cargo test --doc                     # Doc tests only
cargo test --test integration_tests  # Integration tests (uses testcontainers, needs Docker)
cargo test --lib -- config::tests    # Run a specific test module
```

## Architecture

**Entry point:** `Hatchet` (`src/clients/hatchet.rs`) — main client, constructed via `from_env()` or `from_token()`. Provides builder methods for tasks, workflows, and workers.

**Core modules:**
- `src/runnables/` — `Task` and `Workflow` types implementing the `Runnable` trait. Tasks wrap async handler functions with typed input/output. Workflows compose tasks into DAGs with parent/child dependencies.
- `src/worker/` — `Worker` listens for assignments via gRPC (`ActionListener`), dispatches task execution (`TaskDispatcher`), and manages concurrency slots.
- `src/context.rs` — `Context` passed to task handlers. Provides `log()`, `put_stream()`, `parent_output()`, and `filter_payload()`. Log and stream channels are lazily initialized.
- `src/config.rs` — Parses JWT-like API tokens to extract gRPC address and server URL. Reads `HATCHET_CLIENT_TOKEN` and `HATCHET_CLIENT_TLS_STRATEGY` env vars.

**Client layers:**
- `src/clients/grpc/` — Tonic-based gRPC clients (admin, dispatcher, event, workflow). Proto stubs generated at build time into `OUT_DIR`.
- `src/clients/rest/` — OpenAPI-generated REST client. `apis/` and `models/` are auto-generated code. `features/` contains hand-written higher-level clients (e.g., `RunsClient` for workflow run management and streaming).

**Key traits:**
- `Runnable<I, O>` — `run()` (synchronous wait with polling) and `run_no_wait()` (fire-and-forget). Implemented by both `Task` and `Workflow`.
- `Register` — marks a type as registrable on a `Worker` via `add_task_or_workflow()`.

**Builder pattern:** Uses `derive_builder` extensively for `Task`, `Workflow`, `Worker`, and `TriggerWorkflowOptions`.

## Integration Tests

Tests in `tests/integration_tests.rs` spin up Hatchet via testcontainers (Docker required). The `TEST_HATCHET_LITE_VERSION` env var controls which Hatchet version to test against. CI runs a matrix across multiple versions (v0.68.0 through v0.78.0 + latest).

## Re-exports

The crate re-exports `serde`, `serde_json`, `tokio`, and `anyhow` for consumer convenience. Users use `#[serde(crate = "hatchet_sdk::serde")]` on their types.
