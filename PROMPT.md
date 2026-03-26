## Claude Code Prompt

```
You are implementing two missing features in the `hatchet-rust-sdk` crate 
(https://github.com/eswolinsky3241/hatchet-rust-sdk): **concurrency control** 
and **rate limiting** on the `TaskBuilder`. The SDK maintainer has welcomed 
this contribution.

## Your primary references

Before writing any code, read these files in the repo:
- `src/runnables/task.rs` — the `Task` struct and `TaskBuilder` (your main 
  target)
- `src/runnables/workflow.rs` — the `Workflow` and `WorkflowBuilder` (check 
  if concurrency lives here at the workflow level; it may need to go here too)
- `src/worker/worker.rs` — how tasks get registered with the Hatchet engine 
  on worker start; this is where the proto message is built and sent over gRPC
- `api-contracts/` — the bundled protobuf definitions. Find the proto messages 
  for workflow/task registration. Look for fields named `concurrency`, 
  `rate_limits`, or similar on the step/workflow create message types
- `build.rs` — understand how protos are compiled so you know where the 
  generated types live
- Any existing examples in `examples/` that use concurrency or rate limits 
  (there likely aren't any yet, but check)

## What to implement

### 1. Concurrency (workflow-level)

In the Hatchet data model, concurrency is set at the **workflow level**, not 
the step level. Look at how other SDKs model this:

- A `ConcurrencyExpression` with:
  - `expression: String` — a CEL expression evaluated against task input 
    (e.g. `"input.provider_id"`)
  - `max_runs: i32` — maximum concurrent runs for this key
  - `limit_strategy: ConcurrencyLimitStrategy` — enum with variants 
    `CancelInProgress` and `GroupRoundRobin`

Add this to `WorkflowBuilder` (and by extension the standalone `TaskBuilder` 
if the proto supports it at that level — check the proto). The builder method 
should be:

```rust
.concurrency(vec![ConcurrencyExpression {
    expression: "input.provider_id".to_string(),
    max_runs: 2,
    limit_strategy: ConcurrencyLimitStrategy::GroupRoundRobin,
}])
```

### 2. Rate Limiting (task/step-level)

Rate limits live at the **step/task level**. There are two variants:

**Static rate limit** — key is a fixed string known at registration time:
```rust
RateLimit::Static {
    key: "my-api-limit".to_string(),
    units: 1,
}
```

**Dynamic rate limit** — key is a CEL expression evaluated against input at 
runtime:
```rust
RateLimit::Dynamic {
    key_expr: "input.provider_id".to_string(),
    units: 1,              // can be int or CEL expr — start with int
    limit: 10,             // can be int or CEL expr — start with int
    duration: RateLimitDuration::Minute,
}
```

`RateLimitDuration` is an enum: `Second`, `Minute`, `Hour`, `Day`.

Add a `rate_limits(Vec<RateLimit>)` builder method to `TaskBuilder`.

### 3. Wire them into registration

Find the worker startup code where tasks/workflows are registered with the 
Hatchet gRPC server. This is where the `Task` and `Workflow` structs get 
converted into proto messages. Map the new fields through to the corresponding 
proto fields.

If the proto-generated types don't yet expose these fields (they may be 
present in the `.proto` files but not surfaced in the Rust builder), wire 
them directly using the generated prost structs.

## Implementation rules

- Follow the exact patterns already established in `TaskBuilder` — use 
  `derive_builder` the same way existing fields like `execution_timeout`, 
  `retries`, and `cron_triggers` are defined
- All new types (`ConcurrencyExpression`, `ConcurrencyLimitStrategy`, 
  `RateLimit`, `RateLimitDuration`) should be defined in a new file 
  `src/types/flow_control.rs` (or wherever the existing types module is — 
  check the module structure first) and re-exported from `lib.rs`
- Make all new public types implement `Debug`, `Clone`, and `serde` 
  `Serialize`/`Deserialize` following the pattern of existing types in the 
  codebase
- Do not break any existing `TaskBuilder` or `WorkflowBuilder` call sites — 
  all new fields must be `Option<T>` and default to `None`
- After implementing, add an example file `examples/flow_control.rs` that 
  demonstrates a task with both a dynamic rate limit and a concurrency 
  expression, following the style of existing examples in `examples/`
- Run `cargo build` and `cargo clippy` and fix all errors and warnings before 
  finishing

## Context on why this matters

This SDK is missing these features compared to the TypeScript and Python 
Hatchet SDKs, which both support `concurrency` (workflow-level, with 
`dynamic_key`/`expression`, `max_runs`, `limit_strategy`) and `rate_limits` 
(step-level, with static and dynamic key variants). The goal is feature parity 
with those SDKs so Rust users can implement the "Double Lock" pattern: 
concurrency limits on in-flight connections AND rate limits on throughput per 
provider, both keyed dynamically off task input.
```

---

A few notes on what I included and why:

The prompt tells the agent to **read the proto contracts first** before writing anything. That's the most likely failure point — if the agent just guesses at proto field names it'll write code that compiles but sends empty values to the server.

It separates **workflow-level concurrency** from **step-level rate limits** explicitly, because they live in different places in the data model and a naive agent might put both on the same struct.

The `RateLimit` as an enum with `Static` and `Dynamic` variants is a deliberate design suggestion — it matches how the TypeScript SDK models the two patterns and maps cleanly to the proto's `key` vs `key_expr` fields.

--

## Test Plan

---

## Phase 1: Compilation and Type Safety

These don't require a running Hatchet instance.

**1.1 — All new fields are truly optional**
Write a task with zero new fields and confirm it compiles identically to before. This is your non-regression check.

**1.2 — Static rate limit compiles**
```rust
hatchet.task("test-static-rl", handler)
    .rate_limits(vec![RateLimit::Static {
        key: "my-api".to_string(),
        units: 1,
    }])
    .build()
    .unwrap();
```

**1.3 — Dynamic rate limit compiles**
```rust
hatchet.task("test-dynamic-rl", handler)
    .rate_limits(vec![RateLimit::Dynamic {
        key_expr: "input.provider_id".to_string(),
        units: 1,
        limit: 10,
        duration: RateLimitDuration::Minute,
    }])
    .build()
    .unwrap();
```

**1.4 — Concurrency compiles**
```rust
hatchet.task("test-concurrency", handler)
    .concurrency(vec![ConcurrencyExpression {
        expression: "input.provider_id".to_string(),
        max_runs: 2,
        limit_strategy: ConcurrencyLimitStrategy::GroupRoundRobin,
    }])
    .build()
    .unwrap();
```

Finally, ask any questions you need if anything is unclear. 