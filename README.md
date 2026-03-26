# roos

**Rust Orchestration Operating System** — a production-grade AI agent framework.

[![Crates.io](https://img.shields.io/crates/v/roos.svg)](https://crates.io/crates/roos)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)

> **Alpha** — core APIs are stable; some planned features are still in progress.

## What is ROOS?

ROOS is an open-source framework for building autonomous AI agents in Rust. It provides:

- A **reasoning loop** (Reasoning → Action → Observation) wired to any LLM provider
- A **tool system** with JSON-schema validation and the `#[roos::tool]` macro
- **Memory backends**: in-memory, Sled (embedded), Qdrant (vector)
- **Multi-agent coordination** via `RoosAgentBus` and `SupervisorAgent`
- An **HTTP trigger server** (Axum) with Bearer auth and HMAC-SHA256 webhook verification
- A **cron + one-shot scheduler** with configurable retry policies
- **LLM providers**: Anthropic, OpenAI, Groq, Cohere, Qwen (DashScope)
- A **CLI** (`roos new / run / serve / list`) for project scaffolding and execution
- Structured logging and OpenTelemetry-ready observability

## Quick Start

```bash
cargo install roos-cli
roos new my-agent
cd my-agent
roos run --input "Summarise the latest Rust release notes"
```

## Crate Structure

| Crate | Purpose |
| --- | --- |
| `roos` | Re-export facade — one import for everything |
| `roos-core` | `Agent`, `Tool`, `Memory`, `LLMProvider` traits + `RoosAgentBus` |
| `roos-orchestrator` | `ReasoningLoop`, `AgentState` machine, `SystemPromptBuilder` |
| `roos-providers` | Anthropic · OpenAI · Groq · Cohere · Qwen providers |
| `roos-tools` | File I/O · Shell · HTTP · Web search tools |
| `roos-memory` | `InMemoryStore` · `SledMemory` (TTL) |
| `roos-trigger` | Axum HTTP server · Bearer auth · HMAC-SHA256 webhooks |
| `roos-scheduler` | Cron + one-shot scheduler · Sled-backed state · Retry policies |
| `roos-observability` | Structured logging · run-id correlation |
| `roos-macros` | `#[roos::tool]` proc-macro + JSON Schema generation |
| `roos-cli` | `roos` binary: `new` · `run` · `serve` · `list` |

## Example: Define a Tool

```rust
use roos::tool;

#[roos::tool]
async fn search_docs(query: String) -> String {
    format!("Results for: {query}")
}
```

## License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or
[MIT License](LICENSE-MIT) at your option.
