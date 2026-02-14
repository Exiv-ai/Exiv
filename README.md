# Exiv

Exiv (Existence × Virtual) is an AI agent orchestration platform written in Rust. It provides a plugin-based kernel where multiple AI engines, tools, and services communicate through an asynchronous event bus. An admin can control plugin permissions at runtime through a human-in-the-loop approval system.

[![Version](https://img.shields.io/badge/version-0.1.0--alpha.1-blue)]()
[![Tests](https://img.shields.io/badge/tests-76%20passing-brightgreen)]()
[![License](https://img.shields.io/badge/license-BSL%201.1%20→%20MIT%202028-blue)](LICENSE)

## Features

- **Plugin architecture** — AI engines (DeepSeek, Cerebras, etc.) and tools load as independent crates. Plugins can be added, removed, or swapped without rebuilding the kernel.
- **Event-driven kernel** — All inter-plugin communication goes through an async event bus. Events are traced end-to-end with unique IDs.
- **Human-in-the-loop security** — Sensitive operations (permission grants, config changes) require explicit admin approval. All decisions are recorded in an audit log.
- **Python bridge** — Execute Python scripts in a sandboxed subprocess with automatic restart on failure (up to 3 attempts).
- **Proc-macro SDK** — The `#[exiv_plugin]` macro generates plugin manifests, factory boilerplate, and capability registration at compile time.
- **Web dashboard** — React + TypeScript UI served from the kernel binary via `rust-embed`. Connects to the kernel's SSE stream for real-time updates.

## Quick Start

```bash
git clone https://github.com/Exiv-ai/Exiv.git
cd Exiv

cargo build
cargo run --package exiv_core
```

The dashboard is served at http://localhost:8081.

To run tests:

```bash
cargo test
```

For optimized release builds:

```bash
cargo build --release
cargo run --package exiv_core --release
```

## Project Structure

```
exiv_core/          Kernel — event bus, plugin manager, HTTP API, rate limiter
exiv_shared/        SDK — traits (Plugin, ReasoningEngine, Tool, etc.) and shared types
exiv_macros/        Procedural macro for plugin manifest generation
exiv_plugins/       Official plugins (8 crates)
exiv_dashboard/     React/TypeScript web UI (Tauri-compatible)
scripts/            Python bridge runtime, installer
docs/               Architecture docs, changelog, development guide
```

## Plugins

| ID | Type | Description |
|----|------|-------------|
| `mind.deepseek` | Reasoning | DeepSeek R1 |
| `mind.cerebras` | Reasoning | Cerebras Llama 3.3 70B |
| `core.ks22` | Memory | Knowledge Store 2.2 — persistent key-value memory with chronological recall |
| `core.moderator` | Tool | Content moderation |
| `hal.cursor` | HAL | Mouse/keyboard control interface |
| `python.analyst` | Tool | Python code execution via bridge subprocess |
| `python.gaze` | Tool | Gaze tracking (mock mode) |
| `adapter.mcp` | Tool | Model Context Protocol client |
| `vision.screen` | Tool | Screen capture |

## API

**Public endpoints:**

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/metrics` | System metrics |
| GET | `/api/plugins` | Plugin list with manifests |
| GET | `/api/agents` | Agent configurations |
| GET | `/api/events` | SSE event stream |
| POST | `/api/chat` | Send message to an agent |

**Admin endpoints** (rate limited to 10 req/s, requires `X-API-Key` header):

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/system/shutdown` | Graceful shutdown |
| POST | `/api/plugins/:id/config` | Update plugin config |
| POST | `/api/plugins/:id/permissions/grant` | Grant permission to plugin |
| GET | `/api/permissions/pending` | List pending permission requests |
| POST | `/api/permissions/:id/approve` | Approve a request |
| POST | `/api/permissions/:id/deny` | Deny a request |

## Configuration

Copy `.env.example` to `.env` and edit as needed.

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `8081` | HTTP server port |
| `DATABASE_URL` | `sqlite:./exiv_memories.db` | SQLite database path |
| `EXIV_API_KEY` | (none) | Admin API key. Required in release builds. |
| `DEEPSEEK_API_KEY` | (none) | DeepSeek API key |
| `CEREBRAS_API_KEY` | (none) | Cerebras API key |
| `CONSENSUS_ENGINES` | (none) | Comma-separated engine IDs for consensus mode |
| `DEFAULT_AGENT_ID` | `agent.karin` | Default agent for `/api/chat` |
| `EXIV_SKIP_ICON_EMBED` | (none) | Set to `1` to skip icon embedding during dev builds |

## Testing

76 tests (39 unit, 37 integration/plugin).

```bash
cargo test                              # all tests
cargo test --package exiv_core          # kernel only
cargo test --test '*'                   # integration tests only
cargo llvm-cov --package exiv_core --html   # coverage report (requires cargo-llvm-cov)
```

## Security

Admin endpoints are protected by API key authentication (`X-API-Key` header) and per-IP rate limiting (10 req/s, burst 20). Plugin permission grants and denials are written to an append-only audit log in SQLite. Plugins run with minimal permissions by default; elevated permissions require human approval through the `/api/permissions` endpoints. Network access from plugins is restricted to a configurable host whitelist.

## Documentation

- [Architecture](docs/ARCHITECTURE.md) — Design principles, event flow, security model
- [Development](docs/DEVELOPMENT.md) — Coding standards, guardrails, PR process
- [Changelog](docs/CHANGELOG.md) — Development history
- [Plugin Macros](exiv_macros/README.md) — `#[exiv_plugin]` usage and build optimization

## License

Business Source License 1.1. Converts to MIT on 2028-02-14.

You can freely use Exiv for plugin development, internal tools, consulting, education, and small-scale commercial projects. Large-scale commercial deployment (>$100k revenue, >1,000 users, >50 employees, or SaaS) requires prior approval from the licensor. See [LICENSE](LICENSE) for the full terms.

## Links

- [GitHub Issues](https://github.com/Exiv-ai/Exiv/issues)
- [GitHub Discussions](https://github.com/Exiv-ai/Exiv/discussions)
- [Documentation](docs/README.md)
