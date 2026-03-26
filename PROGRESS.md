# ROOS – Progress Tracking

## Phase 1 – Kernel (Months 1–3)

- [x] Task 1: Cargo workspace scaffold — done — 11-crate workspace compiles clean (check/test/fmt/clippy all pass)
- [x] Task 2: roos-core AgentError hierarchy (ROOS-CORE-003) — done — 6 variants, Display + Error impls, 8 tests
- [x] Task 3: roos-core AgentInput/AgentOutput/TokenUsage/ToolCallRecord (ROOS-CORE-002) — done — 4 types, serde/uuid/chrono, TokenUsage::add, 5 tests
- [x] Task 4: roos-core Tool trait + ToolError (ROOS-TOOL-001) — done — async Tool trait (object-safe), 4-variant ToolError, 9 tests (22 total)
- [x] Task 5: roos-core Memory trait + MemoryError (ROOS-MEM-001) — done — async Memory trait (object-safe), ConversationHistory/Message, 4-variant MemoryError, 9 tests (31 total)
- [x] Task 6: roos-core LLMProvider trait + Message/ToolSchema/CompletionConfig/CompletionResponse (ROOS-PROV-001) — done — async LLMProvider (object-safe), 7 types, 5-variant ProviderError, 10 tests (41 total)
- [x] Task 7: roos-core Agent trait (ROOS-CORE-001) — done — async Agent trait (object-safe), name/description/run, 2 tests (43 total)
- [x] Task 8: roos-memory InMemoryStore (ROOS-MEM-004) — done — `RwLock<HashMap>` backend, store/load/append/clear, 7 tests
- [x] Task 9: roos-macros #[roos::tool] proc-macro + JSON Schema (ROOS-TOOL-002) — done — attribute macro generates Tool struct + Box::pin impl + schemars schema, 6 integration tests
- [x] Task 10: roos-orchestrator AgentState enum state machine (ROOS-ORCH-002) — done — 6-state machine with validated transitions + TransitionError, 10 tests
- [x] Task 11: roos-orchestrator reasoning loop (ROOS-ORCH-001) — done — ReasoningLoop with provider/tools/memory, Reasoning→Action→Observation, tool errors non-fatal, 3 tests (13 total)
- [x] Task 12: roos-orchestrator SystemPromptBuilder (ROOS-ORCH-003) — done — builder with identity/tools/custom, inject_into CompletionConfig, 8 tests (21 total)
- [x] Task 13: roos-orchestrator tool input JSON Schema validation (ROOS-TOOL-004) — done — validate_tool_input with jsonschema, wired into ReasoningLoop::execute_tool, 5 tests (26 total)
- [x] Task 14: roos-core roos.toml config + env var interpolation (ROOS-CORE-004) — done — RoosConfig/AgentConfig/ProviderConfig/MemoryConfig, ${VAR} interpolation, 6 tests (49 total)
- [x] Task 15: roos-providers Anthropic Claude provider (ROOS-PROV-002) — done — AnthropicProvider + wire types + map_response + extract_error, 4 tests
- [x] Task 16: roos-providers OpenAI provider (ROOS-PROV-003) — done — OpenAIProvider + Chat Completions wire types + map_response + extract_error, 5 tests (9 total)
- [x] Task 17: roos-trigger Axum HTTP server (ROOS-TRIG-001) — done — TriggerServer with /health /agents /trigger /runs/{id}, AppState RwLock store, 6 tests
- [x] Task 18: roos-trigger Bearer token auth (ROOS-TRIG-004) — done — require_bearer middleware + BearerToken extension, TriggerServer::with_token(), 5 tests (11 total)
- [x] Task 19: roos-cli `roos new` scaffolding (ROOS-CLI-001) — done — scaffold() creates roos.toml/src/agent.rs/.gitignore, 4 tests
- [x] Task 20: roos-cli `roos run` agent invocation (ROOS-CLI-002) — done — reads roos.toml, creates provider + ReasoningLoop, runs synchronously
- [x] Task 21: roos-cli `roos list` agent table (ROOS-CLI-004) — done — load_agents() + table output, 2 tests (6 total in roos-cli)
- [x] Task 22: roos-observability structured logging + run_id correlation (ROOS-OBS-001) — done — init_logging (JSON + EnvFilter, try_init), run_span (info_span with %run_id), 5 tests
- [x] Task 23: roos-tools read_file/write_file/list_directory [tools-fs] (ROOS-TOOL-003) — done — 3 Tool impls feature-gated behind tools-fs, 6 tests
- [x] Task 24: roos-tools execute_shell + allowlist [tools-shell] (ROOS-TOOL-003) — done — ExecuteShellTool with allowlist prefix check, cross-platform cmd/sh dispatch, 6 tests
- [x] Task 25: roos-tools http_get/http_post/search_web [tools-http/web] (ROOS-TOOL-003) — done — HttpGetTool, HttpPostTool (reqwest), SearchWebTool (DuckDuckGo IA API), 6 tests
- [x] Task 26: roos-providers Groq/Cohere/Qwen providers + .env.example (ROOS-PROV-005/006/007) — done — GroqProvider + QwenProvider (via complete_compat), CohereProvider (v2 API), 5 tests (14 total), .env.example
- [x] Task 27: roos-memory SledMemory + TTL expiration (ROOS-MEM-002) — done — lazy-evict TTL via expires_at, serde_json encoding, 5 tests (12 total)
- [x] Task 28: roos-scheduler cron scheduler + Sled-backed task state (ROOS-SCHED-001) — done — CronScheduler (Sled backend), SchedulerError/TaskState/ScheduledTask, 5-field+6-field cron, due_tasks/reschedule, 9 tests
- [x] Task 29: roos-scheduler one-shot scheduled tasks (ROOS-SCHED-002) — done — TaskKind (Cron/OneShot), add_one_shot, reschedule marks OneShot Completed, 13 tests total
- [x] Task 30: roos-scheduler retry policy fixed/exponential (ROOS-SCHED-004) — done — RetryStrategy/RetryPolicy types, record_failure (reschedules or marks Failed), add_task_with_retry/add_one_shot_with_retry, tracing::error on exhaustion, 17 tests total
- [x] Task 31: roos-cli `roos serve` daemon (ROOS-CLI-003) — done — Serve subcommand (--port/--config/--daemonize), TriggerServer::serve_with_shutdown, scheduler tick loop, SIGTERM/SIGINT graceful shutdown, SIGHUP config-reload log (Unix), Ctrl-C (Windows), 3 tests (9 total in roos-cli)
- [x] Task 32: roos-trigger HMAC-SHA256 webhook verification (ROOS-TRIG-002) — done — webhook.rs: verify_hmac_sha256/verify_github_signature/require_webhook_signature middleware, TriggerServer::with_webhook_secret, X-Hub-Signature-256 + X-ROOS-Signature headers, constant-time verify_slice, 19 tests total
- [x] Task 33: roos-core RoosAgentBus multi-agent channel bus (ROOS-MULTI-001) — done — bus.rs: BusMessage/BusEvent/BusError, RoosAgentBus (mpsc direct + broadcast pub/sub), register/send/subscribe/publish, 5 tests (54 total in roos-core)
- [x] Task 34: roos-core SupervisorAgent orchestrator-worker (ROOS-MULTI-002) — done — supervisor.rs: WorkerSpec, SupervisorAgent (LLM decompose → bus dispatch → aggregate), parse_subtasks strips surrounding text, 5 tests (59 total in roos-core)
