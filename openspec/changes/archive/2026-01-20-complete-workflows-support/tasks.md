1.  **Consolidate and Verify Implementation**
    *   [x] Verify `WorkflowsLoader` uses `POST /api/workflows` for creation.
    *   [x] Verify `WorkflowsLoader` uses `PUT /api/workflows/{id}` for updates.
    *   [x] Verify `sanitize_workflow` correctly strips system fields.
    *   [x] Verify `definition` field is retained.

2.  **Tests**
    *   [x] Verify unit tests cover sanitization and loader creation.
    *   [x] Run `cargo test kibana::workflows::loader` to confirm stability.
