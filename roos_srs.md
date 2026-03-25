# ROOS – Software Requirements Specification (SRS)

**Rust Orchestration Operating System**
Document Version: 1.0 | Status: Draft | Framework Version Target: 1.0.0

---

## Table of Contents

1. [Introduction](#1-introduction)
2. [Overall Description](#2-overall-description)
3. [Stakeholders & User Personas](#3-stakeholders--user-personas)
4. [System Architecture Overview](#4-system-architecture-overview)
5. [Functional Requirements](#5-functional-requirements)
   - 5.1 [Core Framework (Layer 0)](#51-core-framework-layer-0)
   - 5.2 [Trigger Engine (Layer 1)](#52-trigger-engine-layer-1)
   - 5.3 [Scheduler Engine (Layer 2)](#53-scheduler-engine-layer-2)
   - 5.4 [Orchestration Engine (Layer 3)](#54-orchestration-engine-layer-3)
   - 5.5 [Tool System](#55-tool-system)
   - 5.6 [Memory & State](#56-memory--state)
   - 5.7 [LLM Provider System](#57-llm-provider-system)
   - 5.8 [Multi-Agent Communication](#58-multi-agent-communication)
   - 5.9 [CLI — roos](#59-cli--roos)
   - 5.10 [Observability & Logging](#510-observability--logging)
   - 5.11 [TUI Dashboard — ROOS TUI](#511-tui-dashboard--roos-tui)
   - 5.12 [Mobile App — ROOS Mobile](#512-mobile-app--roos-mobile)
6. [Non-Functional Requirements](#6-non-functional-requirements)
7. [External Interface Requirements](#7-external-interface-requirements)
8. [Security Requirements](#8-security-requirements)
9. [Data Requirements](#9-data-requirements)
10. [Constraints & Assumptions](#10-constraints--assumptions)
11. [Glossary](#11-glossary)

---

## 1. Introduction

### 1.1 Purpose

This Software Requirements Specification (SRS) defines the functional and non-functional requirements for **ROOS** (Rust Orchestration Operating System), a high-performance, open-source AI agent orchestration framework written in the Rust programming language. This document is intended for:

- Core framework contributors and maintainers
- Integration engineers implementing connectors and providers
- DevRel and documentation authors ensuring accuracy
- Potential enterprise customers evaluating technical fit
- Investors and advisors assessing technical credibility

### 1.2 Scope

ROOS (package name: `roos`, crate family: `roos-*`) is a framework for building autonomous AI agents that combine:

- Large Language Model (LLM) reasoning via pluggable provider adapters
- Tool execution via a typed, schema-generating tool system
- Event-driven triggering via HTTP webhooks and the `roos` CLI
- Scheduled and recurring task management
- Persistent agent memory via embedded and external storage backends
- Multi-agent communication via asynchronous channels
- A unified CLI (`roos`) for scaffolding, running, deploying, and managing agents

ROOS targets Rust 1.75+ (MSRV: Minimum Supported Rust Version) and the Tokio async runtime. It compiles to a single statically-linked binary for all supported targets.

**In scope for v1.0:**

- All three architecture layers (Trigger, Scheduler, Orchestration)
- Core `Agent`, `Tool`, `LLMProvider`, and `Memory` traits
- Anthropic (Claude) and OpenAI provider implementations
- Ollama local inference provider implementation
- Sled-backed embedded state store
- Qdrant vector store connector
- File I/O, shell execution, HTTP request standard tools
- `roos` CLI for project scaffolding, execution, and management
- Structured logging and OpenTelemetry trace export

**Out of scope for v1.0:**

- ROOS Cloud (separate hosted product)
- ROOS Studio (separate GUI product)
- ROOS TUI Dashboard (target: v1.1; depends on ROOS Cloud REST API)
- ROOS Mobile App (target: v1.1; depends on ROOS Cloud REST API)
- Enterprise SSO/RBAC/Audit (ROOS Enterprise tier)
- Connector Marketplace infrastructure
- WASM compilation target
- Windows-native builds (Linux and macOS are v1.0 targets)

### 1.3 Definitions, Acronyms, and Abbreviations

See [Section 11: Glossary](#11-glossary) for full definitions. Key terms used throughout:

- **ROOS** — Rust Orchestration Operating System; the name of the framework and the CLI
- **Agent** — An autonomous unit that uses an LLM to reason and select tools to execute
- **Harness** — The configuration and wiring layer that connects triggers, tools, memory, and providers to an agent
- **Tool** — A typed Rust function exposed to the LLM for execution during the reasoning loop
- **Step** — One iteration of the Reasoning → Action → Observation cycle
- **Provider** — An implementation of the `LLMProvider` trait for a specific LLM service
- **`roos`** — The command-line interface distributed with the ROOS framework

### 1.4 References

- Rust Reference: <https://doc.rust-lang.org/reference/>
- Tokio Documentation: <https://tokio.rs/>
- Anthropic API Reference: <https://docs.anthropic.com/>
- OpenAI API Reference: <https://platform.openai.com/docs/>
- Qdrant Documentation: <https://qdrant.tech/documentation/>
- OpenTelemetry Rust SDK: <https://opentelemetry.io/docs/instrumentation/rust/>
- ROOS Documentation (planned): <https://docs.roos.dev/>

---

## 2. Overall Description

### 2.1 Product Perspective

ROOS is a standalone open-source framework distributed via crates.io. It has no required external runtime dependencies — all state management uses embedded storage by default. External storage backends (Qdrant, PostgreSQL) are optional connectors.

ROOS is designed to be the **operating system substrate** on which agent applications are built — analogous to how Axum is the substrate for web applications, or how Tokio is the substrate for async Rust programs. The "OS" framing is intentional and architectural: ROOS provides scheduling, event handling, process isolation, memory management, and a system-call-like tool interface, mirroring the responsibilities of a traditional operating system but purpose-built for AI agent workloads.

### 2.2 Product Functions (High-Level)

1. Define agents with typed tools and a selected LLM provider
2. Accept triggers from HTTP webhooks, scheduled cron jobs, or `roos run` CLI invocation
3. Execute the Reasoning → Action → Observation loop until a terminal condition
4. Persist conversation history and vector memory across agent runs
5. Emit structured logs and OpenTelemetry traces for observability
6. Communicate results back to trigger sources or downstream systems
7. Scaffold, run, deploy, and manage agent projects via the `roos` CLI

### 2.3 Product Constraints

- Must compile with `cargo build --release` with no unstable features (`#![feature(...)]` forbidden in public API)
- Must not require Docker, external databases, or network access for basic operation (offline-first)
- Binary size must remain under 30MB for a release build with all standard features
- Framework must expose a stable public API following semantic versioning (SemVer) from v1.0 onward
- The `roos` CLI must be distributable as a single static binary independent of the host Rust toolchain

---

## 3. Stakeholders & User Personas

### Persona A: "Alex" – The Rust Systems Developer

**Background:** 5+ years of Rust experience. Works at a company building infrastructure tooling. Has experimented with LangChain but was frustrated by Python's overhead and lack of compile-time safety.

**Goals:**

- Build an agent that monitors build pipelines and auto-files GitHub issues for failures
- Deploy as a systemd daemon on a single EC2 instance with minimal resource footprint
- Use `roos serve` to keep the agent running as a background process

**Pain points with existing tools:**

- LangChain's memory consumption on multi-agent scenarios
- Python runtime errors only discovered in production
- Heavy Docker images for simple agent daemons

**What they need from ROOS:**

- A `#[roos::tool]` derive macro that is no more complex than writing a normal Rust function
- A daemon mode with UNIX signal handling (SIGTERM, SIGHUP) via `roos serve`
- Clear compile-time errors when tool schemas are malformed

---

### Persona B: "Beatrice" – The Platform Engineer

**Background:** Senior platform engineer at a 200-person SaaS company. Runs a team responsible for internal developer tooling, CI/CD, and internal automation. Not a Rust expert but is comfortable reading Rust. Evaluates tools by their operational characteristics, not just developer experience.

**Goals:**

- Build an "Infrastructure Sentinel" agent that auto-responds to PagerDuty alerts
- Needs multi-step reasoning (check logs → SSH to server → assess → act or escalate)
- Needs an audit trail for every agent action (for compliance)
- Wants to use `roos deploy` to push to ROOS Cloud without managing infrastructure

**Pain points:**

- Current Python agent solution crashes under load due to GIL
- Secrets (AWS creds, SSH keys) scattered in multiple places
- No structured observability — just `print()` statements

**What they need from ROOS:**

- Structured logging with correlation IDs per agent run
- OpenTelemetry trace export to their existing Grafana stack
- A clear secrets model (env vars or `roos.toml` config, not hardcoded)
- Reliable retry logic and timeout handling built into the framework

---

### Persona C: "Carlos" – The AI/ML Application Developer

**Background:** 3 years in Python AI/ML, learning Rust. Builds agent-powered products for a startup. Cares deeply about cost-per-agent-run and response latency.

**Goals:**

- Build a "Code Review Agent" that reviews PRs and posts inline comments on GitHub
- Needs to be fast (latency SLA: total response < 15 seconds for a 500-line PR)
- Needs to handle concurrent reviews without resource explosion
- Wants to use `roos run` for quick testing during development

**Pain points:**

- LangChain's overhead adds 800ms+ of framework latency per step
- Running 10 concurrent Python agents consumes >4GB RAM
- Token counting and context window management is manual

**What they need from ROOS:**

- Sub-100ms framework overhead per step (excluding LLM API latency)
- Concurrent agent runs sharing a single Tokio runtime without GIL interference
- Built-in context window management (truncation and summarization strategies)

---

## 4. System Architecture Overview

```
┌──────────────────────────────────────────────────────────────────┐
│                      ROOS Binary                                 │
│              (Rust Orchestration Operating System)               │
│                                                                  │
│  ┌─────────────┐   ┌───────────────┐   ┌──────────────────────┐ │
│  │  Layer 1:   │   │   Layer 2:    │   │      Layer 3:        │ │
│  │  Trigger    │──▶│  Scheduler    │──▶│   Orchestration      │ │
│  │  Engine     │   │  Engine       │   │   Engine             │ │
│  │  (Axum)     │   │  (Tokio-Cron) │   │   (State Machine)    │ │
│  └─────────────┘   └───────────────┘   └──────────┬───────────┘ │
│                                                   │             │
│         ┌─────────────────────────────────────────▼───────────┐ │
│         │                  Agent Runtime                      │ │
│         │  ┌───────────┐  ┌────────────┐  ┌───────────────┐  │ │
│         │  │   Tool    │  │   Memory   │  │   Provider    │  │ │
│         │  │  System   │  │   System   │  │  (LLM API)    │  │ │
│         │  └───────────┘  └────────────┘  └───────────────┘  │ │
│         └─────────────────────────────────────────────────────┘ │
│                                                                  │
│  ┌──────────────────────────────────────────────────────────┐   │
│  │  roos CLI  │  roos new  │  roos run  │  roos deploy  │   │   │
│  └──────────────────────────────────────────────────────────┘   │
└──────────────────────────────────────────────────────────────────┘
```

### 4.1 Crate Organization

ROOS is organized as a Cargo workspace with the following crates:

| Crate | Purpose |
|---|---|
| `roos` | Re-export facade; the crate users add to `Cargo.toml` |
| `roos-core` | Core traits: `Agent`, `Tool`, `LLMProvider`, `Memory` |
| `roos-trigger` | Layer 1: Axum server, webhook handlers |
| `roos-scheduler` | Layer 2: Cron scheduler, task queue |
| `roos-orchestrator` | Layer 3: Reasoning loop, state machine |
| `roos-tools` | Standard tool belt: File I/O, Shell, HTTP |
| `roos-memory` | Sled store, vector connector interfaces |
| `roos-providers` | Anthropic, OpenAI, Ollama implementations |
| `roos-macros` | Derive macros: `#[roos::tool]`, `#[roos::agent]` |
| `roos-observability` | Tracing, logging, OpenTelemetry export |
| `roos-cli` | The `roos` binary — scaffolding, run, deploy, manage |

---

## 5. Functional Requirements

Requirements are identified as `ROOS-[LAYER]-[NUMBER]`. Priority levels: **P0** (must ship v1.0), **P1** (should ship v1.0), **P2** (target v1.1+).

---

### 5.1 Core Framework (Layer 0)

#### ROOS-CORE-001 | Agent Trait | P0

The framework MUST provide an `Agent` trait that defines the minimal interface for an autonomous agent.

**Required methods:**

- `async fn run(&self, input: AgentInput) -> Result<AgentOutput, AgentError>`
- `fn name(&self) -> &str`
- `fn description(&self) -> &str`
- `fn tools(&self) -> &[Box<dyn Tool>]`
- `fn provider(&self) -> &dyn LLMProvider`
- `fn memory(&self) -> &dyn Memory`

**Constraints:**

- The trait MUST be object-safe to enable dynamic dispatch (`Box<dyn Agent>`)
- All methods that access external resources MUST be async
- The trait MUST be `Send + Sync` to support multi-threaded Tokio runtimes

---

#### ROOS-CORE-002 | AgentInput / AgentOutput Types | P0

The framework MUST define canonical input and output types.

**`AgentInput` MUST contain:**

- `content: String` — the human/system message initiating the run
- `context: HashMap<String, serde_json::Value>` — arbitrary key-value context
- `run_id: Uuid` — a unique identifier for this agent run (for tracing correlation)
- `max_steps: Option<usize>` — override for maximum reasoning loop iterations

**`AgentOutput` MUST contain:**

- `content: String` — the agent's final response
- `steps_taken: usize` — number of reasoning-action cycles executed
- `tools_called: Vec<ToolCallRecord>` — audit trail of every tool invocation
- `total_tokens: TokenUsage` — token counts (input, output, total) across the run
- `run_id: Uuid` — echoed from input for correlation

---

#### ROOS-CORE-003 | Error Handling | P0

The framework MUST define a structured error type hierarchy.

**Top-level `AgentError` MUST cover:**

- `ProviderError(String)` — LLM API failure
- `ToolError { name: String, source: Box<dyn Error> }` — tool execution failure
- `MaxStepsExceeded(usize)` — agent exceeded `max_steps`
- `ContextWindowExceeded` — input exceeds provider's context window
- `MemoryError(String)` — state storage failure
- `ConfigurationError(String)` — malformed harness configuration

All error variants MUST implement `std::error::Error` and `Display`. The framework MUST NOT panic in any public API path; all failures MUST return `Result`.

---

#### ROOS-CORE-004 | Configuration System | P0

ROOS MUST support TOML-based configuration for all harness settings.

Configuration file (`roos.toml`) MUST support:

- Provider selection and API key reference (env var name, not raw key)
- Tool enablement/disablement per agent
- Max steps, timeout, and retry configuration
- Trigger endpoint configuration (port, path, auth)
- Scheduler configuration (cron expressions)
- Memory backend selection (sled, sqlite, qdrant)
- Log level and format configuration

All configuration values MUST be overridable via environment variables using the pattern `ROOS_{SECTION}_{KEY}`.

**Example `roos.toml`:**

```toml
[roos]
schema_version = 1

[provider]
name = "anthropic"
model = "claude-sonnet-4-6"
api_key = "${ANTHROPIC_API_KEY}"

[trigger]
port = 8080
auth_token = "${ROOS_TRIGGER_TOKEN}"

[memory]
backend = "sled"
sled_path = ".roos/state"
history_ttl_days = 30

[log]
level = "info"
format = "human"
```

---

### 5.2 Trigger Engine (Layer 1)

#### ROOS-TRIG-001 | HTTP Trigger Server | P0

The trigger engine MUST start an HTTP server on a configurable port (default: 8080) using Axum.

**Required endpoints:**

| Method | Path | Description |
|---|---|---|
| POST | `/trigger/{agent_name}` | Submit a task to a named agent |
| GET | `/health` | Health check — returns 200 OK with build info and ROOS version |
| GET | `/agents` | List registered agents and their status |
| GET | `/runs/{run_id}` | Retrieve status and output of a completed run |

**Constraints:**

- The server MUST handle concurrent requests without blocking (fully async)
- Request handling MUST complete within a configurable timeout (default: 30s for queue acceptance, not agent completion)
- The `/trigger` endpoint MUST return `202 Accepted` with `run_id` immediately; agent execution is asynchronous
- The server MUST validate `Content-Type: application/json` on trigger requests

---

#### ROOS-TRIG-002 | Webhook Signature Verification | P1

The trigger engine SHOULD support HMAC-SHA256 webhook signature verification for:

- GitHub webhooks (`X-Hub-Signature-256` header)
- Generic HMAC (`X-ROOS-Signature` header, custom secret)

When signature verification is enabled for a trigger endpoint, requests with missing or invalid signatures MUST return `401 Unauthorized`.

---

#### ROOS-TRIG-003 | CLI Trigger via `roos run` | P0

The framework MUST support direct CLI invocation of any registered agent via the `roos` command.

```bash
roos run <agent_name> --input "Your task here" [--context key=value]
```

**Constraints:**

- `roos run` MUST block until the agent run completes (synchronous mode)
- Output MUST be printed to stdout in both human-readable and `--json` formats
- Exit codes MUST follow UNIX conventions: 0 = success, 1 = agent error, 2 = config error
- `roos run` MUST read `roos.toml` from the current working directory by default, overridable via `--config`

---

#### ROOS-TRIG-004 | Trigger Authentication | P1

The trigger engine SHOULD support Bearer token authentication on `/trigger` endpoints. Tokens MUST be configured via environment variables, not hardcoded in `roos.toml`.

---

### 5.3 Scheduler Engine (Layer 2)

#### ROOS-SCHED-001 | Cron-Style Scheduling | P0

The scheduler engine MUST support cron-expression-based scheduling for agents using `tokio-cron-scheduler`.

**Supported cron format:** Standard 5-field cron (`minute hour day month weekday`) plus optional seconds field (6-field).

**Required scheduler behaviors:**

- Each scheduled task MUST be associated with a named agent and a static input payload
- The scheduler MUST persist scheduled task state using the embedded Sled store so tasks survive process restarts
- Missed executions (caused by process downtime) MUST be logged as missed but MUST NOT execute retroactively unless `catch_up = true` is set in `roos.toml`

**Example `roos.toml` schedule definition:**

```toml
[[schedule]]
agent = "morning-digest"
cron = "0 8 * * *"
input = "Generate the morning digest"
```

---

#### ROOS-SCHED-002 | One-Shot Scheduled Tasks | P0

The scheduler MUST support one-shot tasks: execute once at a specified RFC 3339 timestamp.

```toml
[[schedule]]
agent = "morning-digest"
at = "2025-01-15T08:00:00Z"
input = "Generate today's digest"
```

---

#### ROOS-SCHED-003 | Task Observability | P1

The scheduler MUST expose task state via `roos status` and the `/health` endpoint:

- Number of scheduled tasks registered
- Last execution time per task
- Next scheduled execution time per task
- Success/failure counts per task (last 100 executions)

---

#### ROOS-SCHED-004 | Retry Policy | P1

Scheduled tasks SHOULD support configurable retry policies:

- `max_retries` (default: 3)
- `retry_delay_seconds` (default: 60)
- `retry_strategy`: `fixed` or `exponential` (default: `exponential`)

On final retry failure, the task MUST emit an error log event and record the failure in the embedded store. `roos logs <agent>` MUST surface these failures.

---

### 5.4 Orchestration Engine (Layer 3)

#### ROOS-ORCH-001 | Reasoning Loop | P0

The orchestration engine MUST implement the core Reasoning → Action → Observation loop.

**Loop definition:**

1. **Reasoning:** Send the current conversation history + system prompt to the LLM provider. Parse the response for either a final answer or a tool call request.
2. **Action:** If the LLM requested a tool call, locate the tool by name, validate the input schema, execute the tool function, and capture the result or error.
3. **Observation:** Append the tool result (or error) to conversation history as a new observation message. Return to step 1.
4. **Termination:** Exit the loop when:
   - The LLM returns a final answer without requesting a tool call
   - `max_steps` is reached (configurable, default: 20)
   - A non-retryable error occurs (provider error, auth failure)

---

#### ROOS-ORCH-002 | State Machine Representation | P0

The orchestration engine MUST model the reasoning loop as a Rust enum state machine.

**Required states:**

```rust
pub enum AgentState {
    Idle,
    Reasoning { step: usize, history: ConversationHistory },
    CallingTool { tool_name: String, input: serde_json::Value },
    ObservingResult { tool_name: String, result: ToolResult },
    Responding { final_answer: String },
    Failed { error: AgentError },
}
```

State transitions MUST be explicit and exhaustively pattern-matched. Invalid state transitions MUST be compile-time impossible.

---

#### ROOS-ORCH-003 | System Prompt Management | P0

The orchestration engine MUST support:

- A configurable base system prompt per agent (from `roos.toml` or code)
- Automatic injection of available tool schemas into the system prompt (when the provider does not support native tool calling)
- A `SystemPromptBuilder` API for programmatic prompt construction

---

#### ROOS-ORCH-004 | Context Window Management | P1

The orchestration engine MUST implement context window management to prevent exceeding the provider's token limit.

**Required strategies (configurable in `roos.toml`):**

- `truncate`: Drop oldest messages (preserving system prompt and most recent N messages)
- `summarize`: Call the LLM to summarize earlier portions of the conversation before truncating (uses one additional LLM call)
- `error`: Return `ContextWindowExceeded` and halt (for strict use cases)

**Default behavior:** `truncate` with a 10% safety margin below the provider's reported context window size.

---

#### ROOS-ORCH-005 | Parallel Tool Execution | P1

When the LLM requests multiple tool calls in a single reasoning step (batch tool calling, as supported by Anthropic's Claude API), the orchestration engine SHOULD execute those tool calls concurrently using `tokio::join!` or `futures::future::join_all`.

**Constraint:** Parallel execution must only apply to tools marked `concurrent_safe = true` in their definition. Tools that modify shared state (e.g., file writes) MUST default to sequential execution.

---

### 5.5 Tool System

#### ROOS-TOOL-001 | Tool Trait | P0

The framework MUST provide a `Tool` trait.

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn input_schema(&self) -> serde_json::Value; // JSON Schema object
    async fn execute(&self, input: serde_json::Value) -> Result<String, ToolError>;
    fn concurrent_safe(&self) -> bool { false } // default: sequential
}
```

---

#### ROOS-TOOL-002 | Tool Derive Macro | P0

The framework MUST provide a `#[roos::tool]` proc-macro that auto-implements the `Tool` trait for a given async Rust function.

**Macro requirements:**

- The function signature MUST be used to auto-generate the JSON Schema for `input_schema()`
- Supported parameter types: `String`, `i64`, `f64`, `bool`, `Vec<T>`, `Option<T>`, `serde::Deserialize` structs
- The macro MUST emit a compile error if a parameter type cannot be represented in JSON Schema
- The `description` attribute on the macro MUST be required
- Parameter-level descriptions MUST be supported via `#[roos_param(description = "...")]`

**Example:**

```rust
#[roos::tool(description = "Read a file from disk and return its contents as a string")]
async fn read_file(
    #[roos_param(description = "Absolute path to the file")]
    path: String,
    #[roos_param(description = "Maximum bytes to read (0 = no limit)")]
    max_bytes: Option<i64>,
) -> Result<String, ToolError> {
    // implementation
}
```

---

#### ROOS-TOOL-003 | Standard Tool Belt | P0

The `roos-tools` crate MUST provide the following pre-built tools, enabled via feature flags:

| Tool Name | Feature Flag | Description |
|---|---|---|
| `read_file` | `tools-fs` | Read file contents |
| `write_file` | `tools-fs` | Write/overwrite file |
| `list_directory` | `tools-fs` | List files in a directory |
| `execute_shell` | `tools-shell` | Execute a shell command; return stdout/stderr |
| `http_get` | `tools-http` | HTTP GET request; return response body |
| `http_post` | `tools-http` | HTTP POST with JSON body |
| `search_web` | `tools-web` | Web search (requires API key config) |

**Security constraints for `execute_shell`:**

- MUST be disabled by default (requires explicit feature flag AND `[tools.shell] enabled = true` in `roos.toml`)
- MUST support an allowlist of permitted commands (default: deny all shell execution unless allowlist is configured)
- MUST NOT execute commands as root unless the process itself is root
- MUST log every shell command executed at INFO level with the `run_id` correlation attribute

---

#### ROOS-TOOL-004 | Tool Input Validation | P0

Before executing any tool, the orchestration engine MUST validate the LLM-supplied input against the tool's JSON Schema. Invalid inputs MUST NOT result in tool execution; instead, an error MUST be returned to the LLM as an observation so it can correct its call.

---

### 5.6 Memory & State

#### ROOS-MEM-001 | Memory Trait | P0

The framework MUST provide a `Memory` trait.

```rust
#[async_trait]
pub trait Memory: Send + Sync {
    async fn load_history(&self, run_id: &Uuid) -> Result<ConversationHistory, MemoryError>;
    async fn save_history(&self, run_id: &Uuid, history: &ConversationHistory) -> Result<(), MemoryError>;
    async fn search_similar(&self, query: &str, top_k: usize) -> Result<Vec<MemoryChunk>, MemoryError>;
    async fn store_chunk(&self, chunk: MemoryChunk) -> Result<(), MemoryError>;
    async fn clear(&self, run_id: &Uuid) -> Result<(), MemoryError>;
}
```

---

#### ROOS-MEM-002 | Sled Embedded Store | P0

The `roos-memory` crate MUST provide a `SledMemory` implementation using the embedded Sled key-value store for conversation history persistence.

**Requirements:**

- No external dependencies or network access required
- Conversation history MUST be serialized as MessagePack or JSON
- The Sled database file path MUST be configurable via `roos.toml` (`memory.sled_path`)
- Must handle concurrent access from multiple agent tasks (Sled is thread-safe)
- Must support TTL-based expiration of old conversation histories (`memory.history_ttl_days`)

---

#### ROOS-MEM-003 | Qdrant Vector Store Connector | P1

The `roos-memory` crate MUST provide an optional `QdrantMemory` implementation behind the `memory-qdrant` feature flag.

**Requirements:**

- Connect to a running Qdrant instance (local or remote) via the official Qdrant Rust client
- `store_chunk` MUST embed text using the configured embedding model before storing
- `search_similar` MUST perform ANN search and return top-k results
- The collection name, embedding model, and vector dimensions MUST be configurable in `roos.toml`

---

#### ROOS-MEM-004 | In-Memory Store | P0

The framework MUST provide an `InMemoryStore` implementation for testing and ephemeral use cases. This store MUST NOT persist data across process restarts and MUST be the default when no memory backend is configured in `roos.toml`.

---

### 5.7 LLM Provider System

#### ROOS-PROV-001 | LLMProvider Trait | P0

The framework MUST define an `LLMProvider` trait.

```rust
#[async_trait]
pub trait LLMProvider: Send + Sync {
    fn name(&self) -> &str;
    fn context_window(&self) -> usize;
    async fn complete(
        &self,
        messages: &[Message],
        tools: &[ToolSchema],
        config: &CompletionConfig,
    ) -> Result<CompletionResponse, ProviderError>;
    async fn count_tokens(&self, messages: &[Message]) -> Result<usize, ProviderError>;
}
```

No provider is privileged in the trait definition. All providers are equal implementations.

---

#### ROOS-PROV-002 | Anthropic Provider | P0

The `roos-providers` crate MUST implement `LLMProvider` for Anthropic's Claude API.

**Supported models (configurable via `roos.toml`, defaults to latest stable):**

- `claude-opus-4-6`
- `claude-sonnet-4-6`
- `claude-haiku-4-5-20251001`

**Required features:**

- Tool calling via Anthropic's native tool use API (not prompt injection)
- Streaming responses (for low-latency first-token delivery)
- API key from environment variable referenced in `roos.toml`
- Retry with exponential backoff on 529 (overloaded) and 5xx responses
- Rate limit detection and transparent queuing

---

#### ROOS-PROV-003 | OpenAI Provider | P0

The `roos-providers` crate MUST implement `LLMProvider` for the OpenAI Chat Completions API.

**Supported models:** `gpt-4o`, `gpt-4o-mini`, `gpt-4-turbo`, `o1`, `o1-mini` (configurable).

**Required features:**

- Function/tool calling via OpenAI's native function calling API
- Streaming support
- API key from environment variable referenced in `roos.toml`
- Compatible with OpenAI-compatible endpoints (Azure OpenAI, Groq, Together AI) via `base_url` config override

---

#### ROOS-PROV-004 | Ollama Local Provider | P1

The `roos-providers` crate MUST implement `LLMProvider` for Ollama local inference.

**Requirements:**

- Connect to a locally running Ollama server (default: `http://localhost:11434`)
- Support any model available in the connected Ollama instance
- Implement tool calling via prompt-based tool injection (Ollama does not natively support tool schemas for all models)
- No API key required; no external network access needed

---

### 5.8 Multi-Agent Communication

#### ROOS-MULTI-001 | Agent Bus | P1

The framework MUST provide a `RoosAgentBus` struct built on Tokio broadcast/mpsc channels that enables multiple agents to communicate within the same process.

**Required capabilities:**

- Agent A can send a task to Agent B and await its result
- Agents can publish events to named topics; other agents can subscribe
- The bus MUST be initialized once and passed by reference to all agents in the harness

---

#### ROOS-MULTI-002 | Supervisor Agent | P1

The framework SHOULD provide a `SupervisorAgent` convenience type that:

- Receives a task
- Uses an LLM to decompose the task into subtasks
- Dispatches subtasks to registered worker agents via the `RoosAgentBus`
- Aggregates results and produces a final output

This implements the "Orchestrator-Worker" multi-agent pattern, enabling ROOS to act as a true operating system for multi-agent workloads.

---

### 5.9 CLI — `roos`

The `roos` CLI is a first-class product distributed as a standalone binary. It is the primary interface between developers and the ROOS framework — for scaffolding, development, testing, and production management.

#### ROOS-CLI-001 | `roos new` — Project Scaffolding | P0

`roos new <project-name>` MUST generate a new ROOS agent project with:

- A `Cargo.toml` with correct ROOS dependencies and feature flags
- A `roos.toml` configuration template with all fields commented and documented
- A `src/main.rs` with a minimal working agent skeleton
- A `src/tools.rs` with a single example `#[roos::tool]` implementation
- A `.env.example` file listing required environment variables
- A `README.md` explaining how to run the agent with `roos run`

---

#### ROOS-CLI-002 | `roos run` — Invoke Agent | P0

`roos run <agent_name> [options]` MUST invoke the named agent synchronously from the CLI.

**Options:**

- `--input <text>` — the agent's input (required unless `--input-file` is used)
- `--input-file <path>` — read input from a file
- `--context <key=value>` — add key-value context (repeatable)
- `--max-steps <n>` — override max_steps for this run
- `--json` — output result as JSON
- `--trace` — enable verbose step-by-step trace output to stderr

**Behavior:**

- Reads `roos.toml` from the current directory (or `--config` path)
- Streams log output to stderr while agent runs (unless `--json`)
- Exits with code 0 on success, 1 on agent error, 2 on configuration error

---

#### ROOS-CLI-003 | `roos serve` — Daemon Mode | P0

`roos serve [options]` MUST start ROOS in long-running server mode with both the trigger HTTP server and the scheduler active.

**Options:**

- `--port <n>` — override HTTP trigger port (default: 8080)
- `--config <path>` — path to `roos.toml` (default: current directory)
- `--daemonize` — fork to background (Linux/macOS only)

**Behavior:**

- Handles SIGTERM and SIGINT for graceful shutdown
- Handles SIGHUP for configuration reload without restart
- Logs to stdout/stderr by default; supports log file output via `roos.toml`

---

#### ROOS-CLI-004 | `roos list` — Show Agents | P0

`roos list` MUST print a formatted table of all agents registered in the current project, with their names, descriptions, configured provider, and tool list.

---

#### ROOS-CLI-005 | `roos status` — Runtime Status | P1

`roos status` MUST show the current runtime status of a running `roos serve` instance:

- Active agents and their current state
- Scheduled tasks and next execution times
- Recent run history (last 10 runs per agent)
- Memory backend status (connected, disk usage)

---

#### ROOS-CLI-006 | `roos logs` — Stream Logs | P1

`roos logs <agent_name> [--run-id <uuid>] [--follow]` MUST stream or display logs for a specific agent or run.

---

#### ROOS-CLI-007 | `roos deploy` — Deploy to ROOS Cloud | P1

`roos deploy` MUST package the current project and deploy it to ROOS Cloud (requires ROOS Cloud account and `ROOS_CLOUD_TOKEN`).

**Behavior:**

- Validates `roos.toml` before deployment
- Builds a release binary
- Uploads to ROOS Cloud and starts the agent
- Returns the deployment URL and run endpoint

---

### 5.10 Observability & Logging

#### ROOS-OBS-001 | Structured Logging | P0

All framework components MUST use the `tracing` crate for structured logging. Log events MUST include:

- `run_id` (for correlation across a single agent run)
- `agent_name`
- `step` (current reasoning loop iteration)
- `tool_name` (when relevant)
- Standard log levels: DEBUG, INFO, WARN, ERROR

Log format MUST be configurable via `roos.toml`: `human` (default, pretty-printed) or `json` (for log aggregation pipelines like Loki or Elastic).

---

#### ROOS-OBS-002 | OpenTelemetry Trace Export | P1

The framework MUST support exporting traces to OpenTelemetry-compatible backends (Jaeger, Tempo, OTLP endpoint) via `tracing-opentelemetry` and `opentelemetry-otlp`.

**Required trace spans:**

- One root span per agent run (`roos.agent.run`)
- One child span per reasoning step (`roos.agent.step`)
- One child span per tool execution (`roos.tool.execute`)
- One child span per LLM provider call (`roos.provider.complete`)

All spans MUST include `run_id` as a trace attribute for end-to-end correlation.

---

#### ROOS-OBS-003 | Metrics | P2

The framework SHOULD expose Prometheus-compatible metrics via a `/metrics` endpoint:

- `roos_runs_total{agent, status}` — counter
- `roos_run_duration_seconds{agent}` — histogram
- `roos_steps_per_run{agent}` — histogram
- `roos_tokens_used_total{agent, provider}` — counter
- `roos_tool_calls_total{agent, tool, status}` — counter

---

### 5.11 TUI Dashboard — ROOS TUI

ROOS TUI is a terminal user interface dashboard built with the `ratatui` crate. It connects to a running `roos serve` instance (local or remote) via the ROOS Cloud REST API and provides full dashboard parity for developers who work in terminal environments, SSH sessions, or headless servers. It is shipped as part of the `roos-cli` crate and launched via `roos tui`.

**Feature parity target:** All views available in the ROOS Cloud web dashboard and ROOS Studio desktop dashboard MUST be representable in the TUI. The TUI is keyboard-driven (vim-style bindings; no mouse required).

---

#### ROOS-TUI-001 | Agent Status Panel | P0

The TUI MUST display a real-time agent status panel showing all registered agents.

**Required columns:**

- Agent name
- Current state (`Idle`, `Running`, `Failed`, `Scheduled`)
- Last run timestamp
- Last run result (success / failure / step count)
- Configured provider and model

**Behavior:**

- Status MUST refresh automatically every 2 seconds (configurable via `--refresh <seconds>`)
- Selected agent MUST be highlighted; pressing `Enter` MUST open the detail view for that agent

---

#### ROOS-TUI-002 | Live Log Streaming Panel | P0

The TUI MUST provide a split-pane log streaming view for a selected agent or run.

**Requirements:**

- Logs MUST stream in real time from the connected `roos serve` instance via the `/runs/{run_id}` REST endpoint (SSE or polling)
- Log lines MUST be color-coded by level: DEBUG (grey), INFO (white), WARN (yellow), ERROR (red)
- The user MUST be able to filter log lines by level using keyboard shortcuts (`d` = debug, `i` = info, `w` = warn, `e` = error)
- Full-text search within the visible log buffer MUST be supported via `/` (forward search, vim-style)
- The log panel MUST support pause (`Space`) and resume scrolling

---

#### ROOS-TUI-003 | Trigger Management View | P1

The TUI MUST provide a trigger management view for each registered agent.

**Required operations:**

- View all configured triggers (HTTP endpoints, cron schedules, one-shot tasks)
- Fire a one-shot trigger for a named agent with an inline input prompt
- View and edit cron schedule expressions (edit mode MUST validate the expression before saving)
- Enable / disable individual triggers without restarting `roos serve`

---

#### ROOS-TUI-004 | Secrets Vault View | P1

The TUI MUST provide a read/write secrets vault view.

**Requirements:**

- Display all secrets registered in the connected ROOS Cloud instance (names only; values MUST be masked by default)
- Support toggling value visibility for a selected secret (`v` key) with a confirmation prompt
- Support adding a new secret via an inline form (name + value)
- Support rotating (updating the value of) an existing secret
- All secret mutations MUST require a confirmation step before submission
- Secret names and values MUST NOT be written to the TUI's scrollback buffer or terminal history

---

#### ROOS-TUI-005 | Usage Analytics View | P1

The TUI MUST provide a usage analytics view rendered as ASCII/Unicode charts.

**Required metrics (per agent, selectable time range: 1h / 24h / 7d / 30d):**

- Token consumption (input + output) as a bar chart
- Step count per run as a histogram
- Run history: list of last 50 runs with outcome, duration, and step count
- Error rate: percentage of failed runs over the selected window

---

#### ROOS-TUI-006 | Multi-Agent Side-by-Side View | P2

The TUI SHOULD support a multi-column layout showing up to 4 agents simultaneously, each with their real-time status and last 10 log lines. Useful for monitoring multi-agent pipelines.

---

#### ROOS-TUI-007 | TUI Connection Configuration | P0

`roos tui` MUST accept connection configuration to reach the `roos serve` instance.

**Options:**

- `--host <url>` — ROOS serve base URL (default: `http://localhost:8080`)
- `--token <token>` — Bearer token for authenticated instances (reads from `ROOS_CLOUD_TOKEN` env var if not provided)
- `--config <path>` — read connection settings from `roos.toml` (default: current directory)

The TUI MUST display a connection error screen (not crash) if the `roos serve` instance is unreachable, with a retry countdown.

---

### 5.12 Mobile App — ROOS Mobile

ROOS Mobile is a cross-platform mobile application (iOS 16+ and Android 13+) built with React Native. It connects to ROOS Cloud via the same REST API as the web dashboard and provides full dashboard parity for on-call engineers and team leads who manage agents from mobile devices.

**Feature parity target:** All views available in the ROOS Cloud web dashboard MUST be available in ROOS Mobile with platform-appropriate UI conventions (native navigation, touch targets, pull-to-refresh).

---

#### ROOS-MOB-001 | Agent Status Screen | P0

The mobile app MUST display an agent status list screen as the home screen.

**Requirements:**

- List all agents registered in the connected ROOS Cloud account
- Display per-agent: name, current state badge (color-coded), last run time, last run result
- Pull-to-refresh MUST trigger a manual status refresh
- Tapping an agent MUST navigate to the agent detail screen
- Background polling interval: 30 seconds (configurable in app settings)

---

#### ROOS-MOB-002 | Push Notifications | P0

The mobile app MUST support push notifications for agent events.

**Notification triggers (user-configurable per agent):**

- Agent run completed (success)
- Agent run failed
- Scheduled job missed execution
- Step count exceeded threshold (configurable per agent)
- Secrets approaching expiry (if expiry dates are configured)

**Requirements:**

- Notifications MUST include: agent name, event type, timestamp, and a one-line summary
- Tapping a notification MUST deep-link to the relevant agent detail screen
- Notification preferences MUST be configurable per-agent in the app's settings screen
- Push delivery MUST use APNs (iOS) and FCM (Android) via ROOS Cloud's notification service

---

#### ROOS-MOB-003 | Live Log Streaming Screen | P0

The mobile app MUST provide a log streaming screen accessible from the agent detail screen.

**Requirements:**

- Stream logs in real time from the ROOS Cloud REST API (SSE or long-polling fallback)
- Color-coded log levels: DEBUG (grey), INFO (default), WARN (amber), ERROR (red)
- Log level filter chips at the top of the screen (tap to toggle each level)
- Full-text search via a search bar (filters visible log lines)
- "Follow" toggle: when on, the view auto-scrolls to the latest log line; when off, the user can scroll freely
- Share log snippet: long-press on a log line MUST offer a share sheet to copy or export the line

---

#### ROOS-MOB-004 | Trigger Management Screen | P1

The mobile app MUST provide a trigger management screen per agent.

**Required operations:**

- View all configured triggers (endpoint URL, cron expression, or one-shot timestamp)
- Fire a one-shot trigger: tap "Run Now", enter input text in a modal, confirm
- View next scheduled execution time for cron triggers
- Enable / disable individual triggers with a toggle switch (requires confirmation dialog)

---

#### ROOS-MOB-005 | Secrets Vault Screen | P1

The mobile app MUST provide a secrets vault screen.

**Requirements:**

- List all secrets by name; values MUST be hidden by default (shown as `••••••••`)
- Reveal a secret value via biometric authentication (Face ID / Touch ID / device PIN) — one secret at a time
- Add a new secret via a modal form (name + value fields; value field is a secure text entry)
- Rotate (update the value of) an existing secret via a modal form
- All secret mutations MUST require biometric re-authentication before submission
- The app MUST NOT cache secret values in device storage or log them to crash reporters

---

#### ROOS-MOB-006 | Usage Analytics Screen | P1

The mobile app MUST provide a usage analytics screen with native charts.

**Required visualizations (per agent, time range selector: 1h / 24h / 7d / 30d):**

- Token consumption line chart (input + output tokens over time)
- Run outcome bar chart (success vs. failure per time bucket)
- Step count distribution chart (histogram)
- Run history list: last 50 runs with outcome badge, duration, step count, and timestamp

---

#### ROOS-MOB-007 | Team Access Controls Screen | P1

The mobile app MUST provide a read-only team access controls screen.

**Requirements:**

- List all team members with their display name, email, and assigned role
- Display role badge: Owner / Admin / Member / Viewer
- Tapping a team member MUST show their permission scope in a bottom sheet
- Role modifications MUST NOT be available in the mobile app (write operations are limited to web/desktop dashboard for security)

---

#### ROOS-MOB-008 | Authentication & Session Management | P0

**Requirements:**

- The app MUST authenticate via OAuth 2.0 / OIDC using the ROOS Cloud identity provider
- Session tokens MUST be stored in the platform secure keychain (iOS Keychain / Android Keystore) — never in AsyncStorage or local files
- The app MUST support biometric re-authentication for sensitive operations (secrets reveal, trigger fire, secret rotation)
- Sessions MUST expire after 30 days of inactivity; the app MUST prompt for re-authentication gracefully
- The app MUST support multiple ROOS Cloud accounts (account switcher in settings)

---

#### ROOS-MOB-009 | Offline State | P1

When the mobile device has no network connectivity, the app MUST:

- Display a persistent "Offline" banner
- Show the last-cached agent status with a "Last updated" timestamp
- Disable all write operations (trigger fire, secret management, toggle) with a clear "Requires connection" message
- Resume real-time streaming automatically when connectivity is restored

---

## 6. Non-Functional Requirements

### 6.1 Performance

| Requirement | Target | Measurement Method |
|---|---|---|
| Framework overhead per reasoning step | < 5ms (P99) | Microbenchmark excluding LLM API latency |
| Binary size (release build, all features) | < 30MB | `cargo build --release`, `wc -c` |
| Cold start time (process startup to first request) | < 100ms | Time from process spawn to `/health` returning 200 |
| Memory footprint (idle, 0 active agents) | < 20MB RSS | `ps aux` on Linux |
| Memory footprint (10 concurrent agent runs) | < 200MB RSS | Load test with 10 parallel `roos run` invocations |
| Concurrent trigger requests without queuing | ≥ 1,000 | `wrk` benchmark on `/trigger` endpoint |

### 6.2 Reliability

- The orchestration engine MUST NOT panic in any condition caused by external input (malformed LLM response, invalid tool input, network failure)
- The scheduler MUST survive process restart without losing scheduled task state (durability via Sled)
- All public APIs MUST be tested with both happy-path and adversarial inputs (empty strings, max-length strings, null values, malformed JSON)

### 6.3 Portability

- MSRV: **Rust 1.75**
- Target triples for v1.0 release builds: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`
- The `roos` CLI binary MUST be distributable without requiring the Rust toolchain on the target machine
- Windows builds (`x86_64-pc-windows-msvc`) are P2 (v1.1 target)

### 6.4 Maintainability

- All public API items MUST have rustdoc documentation with at least one example
- Code coverage MUST remain above 70% (measured by `cargo-tarpaulin`)
- All unsafe code MUST be in a dedicated `unsafe_impls.rs` file with a written safety justification for each `unsafe` block
- `CHANGELOG.md` MUST be maintained in Keep a Changelog format

### 6.5 Backward Compatibility

- From v1.0.0 onward, all breaking changes to the public API MUST be gated behind a semver major version bump
- The `roos.toml` configuration schema MUST be versioned (`schema_version = 1`) and migration guides provided for breaking schema changes

---

## 7. External Interface Requirements

### 7.1 Anthropic API

- Endpoint: `https://api.anthropic.com/v1/messages`
- Authentication: `x-api-key` header from environment variable referenced in `roos.toml`
- Supported API version header: `anthropic-version: 2023-06-01` (configurable for forward compatibility)
- The provider implementation MUST handle `anthropic-beta` headers for experimental features via optional `roos.toml` configuration

### 7.2 OpenAI API

- Endpoint: `https://api.openai.com/v1/chat/completions` (or configurable `base_url` in `roos.toml`)
- Authentication: `Authorization: Bearer` from environment variable

### 7.3 Ollama API

- Default endpoint: `http://localhost:11434/api/chat`
- No authentication required for local deployments
- Endpoint configurable in `roos.toml`

### 7.4 Qdrant API

- Default endpoint: `http://localhost:6333`
- Optional `QDRANT_API_KEY` for authenticated deployments
- Protocol: gRPC (preferred) or REST

### 7.5 OpenTelemetry Collector

- Protocol: OTLP/gRPC (default) or OTLP/HTTP
- Endpoint: configurable via `OTEL_EXPORTER_OTLP_ENDPOINT` (standard OTel env var)

### 7.6 ROOS Cloud API (for `roos deploy`)

- Endpoint: `https://api.roos.dev/v1` (planned)
- Authentication: `ROOS_CLOUD_TOKEN` environment variable
- Protocol: HTTPS/REST with JSON payloads

---

## 8. Security Requirements

### 8.1 Secrets Management

- API keys MUST NOT be stored in `roos.toml` or any file committed to version control
- All secrets MUST be read from environment variables at runtime
- `roos.toml` MUST support `"${ENV_VAR_NAME}"` interpolation syntax for secret references
- `roos new` MUST generate a `.gitignore` that excludes `.env` files and the `.roos/` state directory
- The `execute_shell` tool MUST be disabled by default and require explicit opt-in in `roos.toml`

### 8.2 Input Sanitization

- All LLM-generated inputs to tools MUST be validated against JSON Schema before execution (ROOS-TOOL-004)
- Path traversal attacks via `read_file` / `write_file` MUST be prevented: paths MUST be validated against a configurable `allowed_paths` allowlist in `roos.toml`
- Shell injection via `execute_shell` MUST be prevented: the tool MUST execute commands via `tokio::process::Command` with argument arrays, never shell string interpolation

### 8.3 Network Security

- All outbound HTTP connections (to LLM providers, webhooks) MUST use TLS 1.2 or higher
- Certificate validation MUST be enabled by default and only disableable via explicit `[tls] verify = false` in `roos.toml` (with a WARN log on startup)
- The trigger HTTP server MUST support TLS (via `rustls`) when a certificate and key are provided in `roos.toml`

### 8.4 Authentication

- The trigger server MUST support Bearer token authentication (ROOS-TRIG-004)
- Tokens MUST be compared using constant-time comparison to prevent timing attacks (`subtle` crate or equivalent)

---

## 9. Data Requirements

### 9.1 Conversation History Schema

```json
{
  "run_id": "uuid-v4",
  "agent_name": "string",
  "created_at": "ISO 8601 timestamp",
  "messages": [
    {
      "role": "system | user | assistant | tool",
      "content": "string",
      "tool_call_id": "string | null",
      "tool_name": "string | null",
      "timestamp": "ISO 8601 timestamp"
    }
  ]
}
```

### 9.2 Tool Call Record Schema

```json
{
  "tool_name": "string",
  "input": "JSON object",
  "output": "string | null",
  "error": "string | null",
  "duration_ms": "integer",
  "step": "integer",
  "timestamp": "ISO 8601 timestamp"
}
```

### 9.3 Data Retention

- Default conversation history retention: 30 days (configurable via `memory.history_ttl_days` in `roos.toml`)
- Log retention is the responsibility of the operator's logging infrastructure; ROOS does not manage log storage
- The embedded Sled store MUST support compaction (`sled::Db::flush()`) to reclaim disk space; `roos status` MUST report current store size

---

## 10. Constraints & Assumptions

### Constraints

- ROOS requires a working Rust toolchain (1.75+) and `cargo` to build from source; pre-built `roos` CLI binaries are available for users who do not want to install Rust
- The Tokio async runtime is the only supported async executor; `async-std` and `smol` are not supported
- The framework does not manage LLM API costs; operators are responsible for monitoring and limiting their own API usage
- Multi-process (distributed) agent coordination is out of scope for v1.0; all agents run within a single process

### Assumptions

- Users of the Rust library are expected to have basic Rust familiarity (cargo, crates, traits)
- Users of the `roos` CLI are not required to know Rust; they interact only with `roos.toml` and CLI commands
- LLM provider API endpoints are network-reachable from the deployment environment (unless using Ollama local)
- For production deployments, the operator is responsible for process supervision (systemd, supervisord, Kubernetes)
- `roos.toml` is present in the working directory at startup, or its path is provided via `ROOS_CONFIG` environment variable

---

## 11. Glossary

| Term | Definition |
|---|---|
| **ROOS** | Rust Orchestration Operating System — the name of the framework, the CLI, and the company |
| **`roos`** | The command-line interface; the primary tool for interacting with the ROOS framework |
| **`roos.toml`** | The TOML configuration file that defines an agent harness — providers, tools, memory, triggers, and schedules |
| **Agent** | An autonomous software entity that uses an LLM to reason about a task and select tools to accomplish it |
| **Agent Harness** | The complete configuration and wiring that defines an agent: its tools, provider, memory, triggers, and system prompt |
| **ANN** | Approximate Nearest Neighbor — a class of algorithms for fast vector similarity search |
| **APNs** | Apple Push Notification service — Apple's infrastructure for delivering push notifications to iOS devices |
| **FCM** | Firebase Cloud Messaging — Google's infrastructure for delivering push notifications to Android devices |
| **ROOS TUI** | The terminal user interface dashboard for ROOS, built with `ratatui`; launched via `roos tui`; connects to a running `roos serve` instance |
| **ROOS Mobile** | The ROOS iOS and Android mobile application, built with React Native; connects to ROOS Cloud |
| **ratatui** | A Rust crate for building terminal user interfaces; used to implement ROOS TUI |
| **React Native** | A cross-platform mobile framework used to implement ROOS Mobile (iOS and Android from a shared codebase) |
| **SSE** | Server-Sent Events — an HTTP-based protocol for server-to-client streaming; used by ROOS TUI and Mobile for real-time log delivery |
| **Crate** | A Rust compilation unit, analogous to a package in other languages |
| **Fearless Concurrency** | Rust's memory model guarantee that data races are impossible at compile time |
| **GIL** | Global Interpreter Lock — a CPython mechanism that prevents true parallelism within a Python process |
| **Harness** | See Agent Harness |
| **HMAC** | Hash-based Message Authentication Code — used for webhook signature verification |
| **LLM** | Large Language Model — a neural network trained on large text corpora, capable of following instructions and reasoning |
| **MSRV** | Minimum Supported Rust Version — the oldest Rust release guaranteed to compile the crate |
| **Observation** | In the reasoning loop: the result of a tool execution, returned to the LLM as input for the next reasoning step |
| **Proc-macro** | A Rust procedural macro — code that runs at compile time to generate or transform Rust source code |
| **Provider** | An implementation of `LLMProvider` for a specific LLM service (Anthropic, OpenAI, Ollama, etc.) |
| **Reasoning Loop** | The iterative Reasoning → Action → Observation cycle executed by an agent until a terminal condition |
| **`RoosAgentBus`** | The inter-agent communication bus — a Tokio channel abstraction for multi-agent ROOS deployments |
| **Sled** | An embedded key-value store written in Rust, used for persistent state without external dependencies |
| **Step** | One complete iteration of the reasoning loop |
| **Tool** | A typed Rust function exposed to an LLM for execution; the agent's "system call" interface to the real world |
| **Token** | The atomic unit of text processed by an LLM; pricing and context windows are measured in tokens |
| **Tokio** | The most widely used async runtime for Rust, providing async I/O, timers, and task scheduling |
| **Trait** | A Rust language feature defining a shared interface that types can implement, analogous to an interface in other languages |

---

*This SRS is a living document. Requirements marked P2 or without a version assignment are candidates for future releases. All requirements are subject to revision based on community feedback and implementation learnings.*

*Document Owner: ROOS Core Team*
*Last Updated: 2026-03-25*
