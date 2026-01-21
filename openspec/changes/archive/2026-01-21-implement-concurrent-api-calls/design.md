# Design: Implement Concurrent API Calls

## Architecture

### Global Concurrency Control
The concurrency limit will be enforced at the lowest level of the Kibana API interaction: the `KibanaClient`.

- **Semaphore**: A `tokio::sync::Semaphore` wrapped in an `Arc` will be added to the `KibanaClient` struct.
- **Shared State**: Since `KibanaClient` is cloned when scoping to a space, the `Arc<Semaphore>` ensures that all client instances (root or space-scoped) share the same concurrency pool.
- **Acquisition**: In `KibanaClient::request_raw`, a permit will be acquired before the `reqwest` call is made. The permit is held until the response is received (or fails).

### Concurrent Execution Patterns
To take advantage of the semaphore, high-level operations must be refactored from sequential loops to concurrent execution.

#### Item-level Concurrency (Extractors/Loaders)
Extractors that fetch multiple items by ID (e.g., `WorkflowsExtractor`, `ToolsExtractor`, `AgentsExtractor`) will be updated to use `futures::stream::StreamExt::buffer_unordered` or `tokio::task::JoinSet` to trigger multiple requests simultaneously.

#### Space-level Concurrency (CLI)
The main loops in `src/cli.rs` that iterate over spaces will be updated to process spaces concurrently. Each space's operations (pulling saved objects, workflows, agents, tools) can be executed as a separate task.

## Configuration
- **Environment Variable**: `KIBANA_MAX_REQUESTS`
- **Default Value**: 8
- **Implementation**: Loaded in `src/cli.rs:load_kibana_client` and passed to `KibanaClient::try_new`.

## Trade-offs and Considerations
- **Kibana Load**: High concurrency might stress Kibana. The default of 8 is a conservative starting point.
- **Error Handling**: When running concurrently, one failing request shouldn't necessarily cancel others, but the overall operation should report failure if any critical step fails. We will continue using `eyre` for error reporting.
- **Logging**: Concurrent operations might interleave log messages. We should ensure logs remain useful. Existing use of `log` and `owo-colors` is compatible with concurrency, though interleaving is possible.
