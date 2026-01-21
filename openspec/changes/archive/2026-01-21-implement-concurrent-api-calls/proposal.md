# Proposal: Implement Concurrent API Calls

Implement a global concurrency limit for Kibana API requests to significantly speed up operations that involve multiple assets (spaces, tools, agents, or workflows) while preventing overwhelming the Kibana server.

## Problem
Currently, many operations in `kibana-object-manager` are sequential. For example, when pulling workflows for a space, each workflow is fetched one by one. When processing multiple spaces, each space is processed one by one. This leads to slow synchronization times, especially when dealing with many items.

## Solution
1.  Introduce a configurable `MAX_REQUESTS` setting (via `KIBANA_MAX_REQUESTS` environment variable, defaulting to 8).
2.  Implement a global concurrency limit in `KibanaClient` using a shared `tokio::sync::Semaphore`.
3.  Refactor extractors and loaders to perform item-level operations concurrently.
4.  Refactor CLI workflows to process spaces and asset types concurrently where safe.

## Impact
- **Performance**: Significant reduction in total execution time for `pull` and `push` operations.
- **Reliability**: Controlled concurrency prevents hitting Kibana's rate limits or overloading its resources.
- **Backward Compatibility**: Existing environment variables and CLI commands remain unchanged, with a new optional configuration.
