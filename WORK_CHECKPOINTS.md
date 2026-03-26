# Work Checkpoints — Flow Control (Concurrency + Rate Limiting)

## Status: ✅ Implementation Complete

`cargo build` and `cargo clippy` both pass with zero new errors or warnings.

---

## What Was Implemented

### 1. New types module: `src/runnables/flow_control.rs`

Defines four user-facing types, all deriving `Debug`, `Clone`, `Serialize`, `Deserialize`:

| Type | Purpose |
|---|---|
| `ConcurrencyExpression` | Workflow-level concurrency control — CEL expression, max runs, and limit strategy |
| `ConcurrencyLimitStrategy` | Enum: `CancelInProgress`, `GroupRoundRobin`, `CancelNewest` |
| `RateLimit` | Task-level rate limiting — enum with `Static { key, units }` and `Dynamic { key_expr, units, limit, duration }` |
| `RateLimitDuration` | Enum: `Second`, `Minute`, `Hour`, `Day`, `Week`, `Month`, `Year` |

Each type has a `to_proto()` method that maps to the corresponding prost-generated proto type from `v1/workflows.proto`.

### 2. `TaskBuilder` changes (`src/runnables/task.rs`)

Added two new optional fields (defaulting to empty `Vec`s):

- `rate_limits: Vec<RateLimit>` — wired into `to_task_proto()` → populates `CreateTaskOpts.rate_limits`
- `concurrency: Vec<ConcurrencyExpression>` — wired into `to_standalone_workflow_proto()` → populates `CreateWorkflowVersionRequest.concurrency_arr`

### 3. `WorkflowBuilder` changes (`src/runnables/workflow.rs`)

Added one new optional field:

- `concurrency: Vec<ConcurrencyExpression>` — wired into `to_proto()` → populates `CreateWorkflowVersionRequest.concurrency_arr`

### 4. Re-exports (`src/runnables/mod.rs`, `src/lib.rs`)

All four types are publicly re-exported from the crate root so consumers can do:
```rust
use hatchet_sdk::{ConcurrencyExpression, ConcurrencyLimitStrategy, RateLimit, RateLimitDuration};
```

### 5. Example: `examples/flow_control.rs`

Demonstrates the "Double Lock" pattern — a standalone task with both:
- Workflow-level concurrency (max 2 concurrent runs per `input.provider_id`, group round-robin)
- Task-level dynamic rate limiting (10 units/minute per `input.provider_id`, consuming 1 unit per run)

Registered in `Cargo.toml` as the `flow_control` example.

---

## Key Discoveries from Proto Analysis

The proto file `api-contracts/protos/v1/workflows.proto` already had all the fields we needed:

- **`CreateWorkflowVersionRequest.concurrency_arr`** (field 12) — `repeated Concurrency` — this is the non-deprecated workflow-level concurrency field
- **`CreateTaskOpts.rate_limits`** (field 7) — `repeated CreateTaskRateLimit` — task-level rate limits
- **`CreateTaskRateLimit`** has both `key` (static) and `key_expr` (dynamic CEL) fields, plus `units`, `units_expr`, `limit_values_expr`, and `duration`
- **`Concurrency`** message has `expression`, `max_runs`, and `limit_strategy`
- **`ConcurrencyLimitStrategy`** enum includes `CANCEL_IN_PROGRESS`, `GROUP_ROUND_ROBIN`, `CANCEL_NEWEST` (plus deprecated `DROP_NEWEST` and `QUEUE_NEWEST`)
- **`RateLimitDuration`** enum includes `SECOND` through `YEAR`

The existing code was already setting `rate_limits: vec![]` and `concurrency_arr: vec![]` in the proto construction — we just needed to populate them from builder fields.

---

## Files Changed

| File | Change |
|---|---|
| `src/runnables/flow_control.rs` | **Created** — new types with proto conversions |
| `src/runnables/mod.rs` | **Modified** — added `flow_control` module + re-exports |
| `src/runnables/task.rs` | **Modified** — added `rate_limits` and `concurrency` fields, wired into proto |
| `src/runnables/workflow.rs` | **Modified** — added `concurrency` field, wired into proto |
| `src/lib.rs` | **Modified** — added public re-exports |
| `examples/flow_control.rs` | **Created** — "Double Lock" example |
| `Cargo.toml` | **Modified** — registered example |

---

## Build Environment Notes

- The repo uses **Rust edition 2024** and relies on nightly features (`let` chains in `workflow.rs`, `is_multiple_of` in `utils.rs`), so it requires a nightly or very recent stable Rust toolchain.
- `protoc` must be installed (`brew install protobuf`) for the build script to compile protos.
- Pre-existing clippy warnings exist on generated proto types (`enum_variant_names`) and existing code (`type_complexity`, `wrong_self_convention`) — none relate to our changes.
