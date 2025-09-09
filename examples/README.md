# Running code examples

The example binaries define tasks showcasing different Hatchet functionality supported by the SDK. To run an exmaple, store your Hatchet API token in a `.env` file in the project root:

```
HATCHET_CLIENT_TOKEN=xxx
```

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
