# Exiv

Exiv (Existence × Virtual) is an AI agent orchestration platform written in Rust. It provides a plugin-based kernel where multiple AI engines, tools, and services communicate through an asynchronous event bus. An admin can control plugin permissions at runtime through a human-in-the-loop approval system.

[![Version](https://img.shields.io/badge/version-B1-blue)]()
[![Tests](https://img.shields.io/badge/tests-78%20passing-brightgreen)]()
[![License](https://img.shields.io/badge/license-BSL%201.1%20→%20MIT%202028-blue)](LICENSE)

## Features

- **Plugin architecture** — AI engines (DeepSeek, Cerebras, etc.) and tools load as independent crates. Plugins can be added, removed, or swapped without rebuilding the kernel.
- **Event-driven kernel** — All inter-plugin communication goes through an async event bus. Events are traced end-to-end with unique IDs.
- **Human-in-the-loop security** — Sensitive operations (permission grants, config changes) require explicit admin approval. All decisions are recorded in an audit log.
- **Python bridge** — Execute Python scripts in a sandboxed subprocess with automatic restart on failure (up to 3 attempts).
- **Proc-macro SDK** — The `#[exiv_plugin]` macro generates plugin manifests, factory boilerplate, and capability registration at compile time.
- **Web dashboard** — React + TypeScript UI served from the kernel binary via `rust-embed`. Connects to the kernel's SSE stream for real-time updates.

## Installation

### Pre-built Binary

Download and install the latest release with a single command:

```bash
curl -fsSL https://raw.githubusercontent.com/Exiv-ai/Exiv/master/scripts/install.sh | bash
```

This detects your platform, downloads the correct binary, and runs the built-in installer. Customize with environment variables:

```bash
EXIV_PREFIX=/usr/local EXIV_SERVICE=true \
  curl -fsSL https://raw.githubusercontent.com/Exiv-ai/Exiv/master/scripts/install.sh | bash
```

Windows users: download the `.zip` from [Releases](https://github.com/Exiv-ai/Exiv/releases) and run `exiv_system.exe install --service`.

### From Source

```bash
git clone https://github.com/Exiv-ai/Exiv.git
cd Exiv

cargo build --release
./target/release/exiv_system install --prefix /opt/exiv --service
```

For development (without installing):

```bash
cargo build
cargo run --package exiv_core
```

### Management

```bash
exiv_system service start       # start the service
exiv_system service stop        # stop the service
exiv_system service status      # check status
exiv_system uninstall           # remove installation
```

The dashboard is served at http://localhost:8081.

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

**Rust plugins** (compiled as crates):

| ID | Type | Description |
|----|------|-------------|
| `mind.deepseek` | Reasoning | DeepSeek R1 |
| `mind.cerebras` | Reasoning | Cerebras Llama 3.3 70B |
| `core.ks22` | Reasoning | Knowledge Store 2.2 — persistent key-value memory with chronological recall |
| `core.moderator` | Reasoning | Consensus moderator for collective intelligence |
| `hal.cursor` | HAL | Mouse/keyboard control interface |
| `bridge.python` | Reasoning | Python subprocess bridge with self-healing restart |
| `adapter.mcp` | Skill | Model Context Protocol client |
| `vision.screen` | Vision | Screen capture and analysis |

**Python plugins** (loaded through `bridge.python`):

| ID | Type | Description |
|----|------|-------------|
| `python.analyst` | Reasoning | Data analysis agent with external data fetching |
| `python.gaze` | Vision | Webcam-based eye tracking via MediaPipe |

## API

**Public endpoints:**

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/metrics` | System metrics |
| GET | `/api/plugins` | Plugin list with manifests |
| GET | `/api/plugins/:id/config` | Plugin configuration |
| GET | `/api/agents` | Agent configurations |
| GET | `/api/history` | Event history |
| GET | `/api/memories` | Memory entries |
| GET | `/api/events` | SSE event stream |
| GET | `/api/permissions/pending` | Pending permission requests |
| GET | `/api/system/version` | Current version info |
| GET | `/api/system/update/check` | Check for updates |
| POST | `/api/chat` | Send message to an agent |

**Admin endpoints** (rate limited to 10 req/s, requires `X-API-Key` header):

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/system/shutdown` | Graceful shutdown |
| POST | `/api/system/update/apply` | Apply pending update |
| POST | `/api/plugins/apply` | Bulk enable/disable plugins |
| POST | `/api/plugins/:id/config` | Update plugin config |
| POST | `/api/plugins/:id/permissions/grant` | Grant permission to plugin |
| POST | `/api/agents` | Create agent |
| POST | `/api/agents/:id` | Update agent |
| POST | `/api/events/publish` | Publish event to bus |
| POST | `/api/permissions/:id/approve` | Approve a request |
| POST | `/api/permissions/:id/deny` | Deny a request |

## Configuration

All settings have sensible defaults. Optionally copy `.env.example` to `.env` to override them.

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | `8081` | HTTP server port |
| `DATABASE_URL` | `sqlite:{exe_dir}/data/exiv_memories.db` | SQLite database path (relative to binary location) |
| `EXIV_API_KEY` | (none) | Admin API key. Required in release builds. |
| `DEEPSEEK_API_KEY` | (none) | DeepSeek API key |
| `CEREBRAS_API_KEY` | (none) | Cerebras API key |
| `CONSENSUS_ENGINES` | `mind.deepseek,mind.cerebras` | Comma-separated engine IDs for consensus mode |
| `DEFAULT_AGENT_ID` | `agent.exiv_default` | Default agent for `/api/chat` |
| `EXIV_SKIP_ICON_EMBED` | (none) | Set to `1` to skip icon embedding during dev builds |
| `RUST_LOG` | `info` | Log level filter |
| `MAX_EVENT_DEPTH` | `10` | Maximum event cascading depth |
| `PLUGIN_EVENT_TIMEOUT_SECS` | `30` | Plugin event handler timeout |
| `CORS_ORIGINS` | (none) | Allowed CORS origins (comma-separated) |
| `ALLOWED_HOSTS` | (none) | Network whitelist for plugin access |
| `EXIV_UPDATE_REPO` | (none) | GitHub `owner/repo` for update distribution |

## Testing

78 tests (39 unit, 39 integration/plugin).

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

## Note

Most of the code and documentation in this project was written with the assistance of AI. If you find any issues, inaccuracies, or bugs, please open an [issue](https://github.com/Exiv-ai/Exiv/issues) and we will address it.
