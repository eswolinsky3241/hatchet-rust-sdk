# Running code examples

The example binaries define tasks showcasing different Hatchet functionality supported by the SDK. To run an example, store your Hatchet API token in a `.env` file in the project root:

```
HATCHET_CLIENT_TOKEN=xxx
```

## General examples

Tasks must be registered with Hatchet by a worker to be run successfully. Before running any of the task examples, start the worker binary:
```
cargo run --example worker
```

With the worker listening, run a task in a separate terminal instance:
```
cargo run --example simple
```
Hatchet should assign your task to the worker you started. After completion, the task output will be printed to `stdout`:
```
Result: hello, world!
```

Other examples runnable against the generic worker: `dag`, `error`, `dynamic_child_spawning`, `input_json_schema`, `streaming`, `concurrency`, `rate_limits`, `flow_control`, and `workflow_concurrency`.
