# VERS SYSTEM

> **V**ersatile **E**vent-driven **R**easoning **S**ystem

A production-ready AI agent orchestration platform built in Rust, featuring plugin-based architecture, event-driven communication, and human-in-the-loop security controls.

[![Version](https://img.shields.io/badge/version-0.1.0--alpha.1-blue)]()
[![Tests](https://img.shields.io/badge/tests-45%20passing-brightgreen)]()
[![Code Quality](https://img.shields.io/badge/quality-90%2F100%20(A)-brightgreen)]()
[![License](https://img.shields.io/badge/license-MIT-blue)]()

## ✨ Features

- **🔌 Plugin Architecture**: Hot-swappable AI engines (DeepSeek, Cerebras, etc.)
- **🛡️ Security-First**: Rate limiting, audit logging, human-in-the-loop approvals
- **⚡ Event-Driven**: Actor model with async event bus
- **🐍 Python Bridge**: Execute Python code with sandboxed capabilities
- **🎯 Type-Safe Macros**: Automatic manifest generation and capability registration
- **📊 Real-Time Dashboard**: React + TypeScript web interface
- **🔄 Self-Healing**: Auto-restart on plugin crashes (max 3 attempts)

## 🚀 Quick Start

```bash
# Clone repository
git clone <repository-url>
cd vers_project

# Build (development)
cargo build

# Run kernel
cargo run --package vers_core

# Run tests
cargo test

# Build optimized release
cargo build --release
cargo run --package vers_core --release
```

**Access Dashboard:** http://localhost:8081

## 📂 Project Structure

```
vers_project/
├── vers_core/          # Kernel: Event bus, plugin manager, API server
├── vers_shared/        # SDK: Common interfaces for plugins
├── vers_plugins/       # Official plugins (DeepSeek, Cerebras, MCP, etc.)
├── vers_macros/        # Procedural macros for plugin development
├── vers_dashboard/     # React/TypeScript web UI
├── scripts/            # Python bridge runtime and utilities
└── docs/               # Architecture, development guides, changelog
```

## 🔌 Available Plugins

| Plugin | Type | Description |
|--------|------|-------------|
| `mind.deepseek` | Reasoning | DeepSeek R1 reasoning engine |
| `mind.cerebras` | Reasoning | Cerebras Llama 3.3 70B engine |
| `core.ks22` | Memory | Knowledge Store 2.2 (persistent memory) |
| `core.moderator` | Tool | Content moderation engine |
| `hal.cursor` | HAL | Cursor control interface |
| `python.analyst` | Tool | Python code execution bridge |
| `python.gaze` | Tool | Gaze tracking (mock mode) |
| `adapter.mcp` | Tool | Model Context Protocol adapter |
| `vision.screen` | Tool | Screen capture and vision |

## 📡 API Endpoints

### Core
- `GET /api/metrics` - System metrics
- `GET /api/plugins` - Plugin list with manifests
- `GET /api/agents` - Agent configurations
- `GET /api/events` - Server-Sent Events stream
- `POST /api/chat` - Send message to agent

### Admin (Rate Limited: 10 req/s)
- `POST /api/system/shutdown` - Graceful shutdown
- `POST /api/plugins/:id/config` - Update plugin config
- `POST /api/plugins/:id/permissions/grant` - Grant permission

### Human-in-the-Loop (Phase 6)
- `GET /api/permissions/pending` - Pending permission requests
- `POST /api/permissions/:id/approve` - Approve request
- `POST /api/permissions/:id/deny` - Deny request

## 🛡️ Security Features

1. **Rate Limiting**: 10 req/s per IP, burst 20 (admin endpoints)
2. **Audit Logging**: All permission grants/denials tracked with timestamps
3. **Human Approval**: Sensitive operations require admin confirmation
4. **Capability Isolation**: Plugins run with minimal permissions
5. **DNS Rebinding Protection**: IP whitelist validation
6. **Authentication**: API key required for admin endpoints

## 📜 Documentation

**Quick Links:**
- **[Architecture](docs/ARCHITECTURE.md)** - Design principles, security framework
- **[Development](docs/DEVELOPMENT.md)** - Coding standards, guardrails
- **[Changelog](docs/CHANGELOG.md)** - Phase 1 → Phase 6 history
- **[Quality Audit](docs/CODE_QUALITY_AUDIT.md)** - Code quality tracking (65 → 90+)

**Plugin Development:**
- **[vers_macros README](vers_macros/README.md)** - Macro usage, optimization guide

## 🧪 Testing

```bash
# Run all tests
cargo test

# Run specific package tests
cargo test --package vers_core
cargo test --package vers_shared

# Run integration tests
cargo test --test '*'

# With coverage (requires cargo-llvm-cov)
cargo llvm-cov --package vers_core --html
```

**Test Coverage:**
- **Unit Tests**: 34 (handlers, db, capabilities, middleware, config)
- **Integration Tests**: 11 (event cascading, security, memory)
- **Total**: 45 tests ✅

## ⚙️ Configuration

**Environment Variables:**

```bash
# Database
DATABASE_URL=sqlite:./vers_memories.db

# Default Agent
DEFAULT_AGENT_ID=agent.karin

# API Keys (optional, for AI engines)
DEEPSEEK_API_KEY=your_key_here
CEREBRAS_API_KEY=your_key_here

# Admin Authentication (required in release mode)
VERS_API_KEY=your_admin_key

# Consensus Engines (Phase 6)
CONSENSUS_ENGINES=mind.deepseek,mind.cerebras

# Build Optimization (Phase 6)
VERS_SKIP_ICON_EMBED=1  # Faster dev builds
```

## 🏗️ Build Optimization

**Development (Fast Incremental Builds):**
```bash
export VERS_SKIP_ICON_EMBED=1
cargo build
```

**Production (Full Icon Embedding):**
```bash
unset VERS_SKIP_ICON_EMBED
cargo build --release
```

## 🎯 Current Status (Phase 6 Complete)

**Version:** 0.1.0-alpha.1
**Code Quality:** 90+/100 (A)
**Design Compliance:** 95+/100 (A)

**Recent Achievements:**
- ✅ Human-in-the-loop permission system
- ✅ Rate limiting & DoS protection
- ✅ Comprehensive audit logging
- ✅ Self-healing Python bridge
- ✅ Build optimization (VERS_SKIP_ICON_EMBED)
- ✅ International accessibility (35 comments translated)

## 🤝 Contributing

See [DEVELOPMENT.md](docs/DEVELOPMENT.md) for:
- Coding standards and style guide
- Development guardrails (DO NOT list)
- Testing requirements
- Pull request process

## 📄 License

[Specify license here - MIT/Apache 2.0/etc.]

## 🔗 Links

- **Documentation**: [docs/README.md](docs/README.md)
- **Dashboard**: http://localhost:8081 (when running)
- **Issue Tracker**: [GitHub Issues]
- **Discussions**: [GitHub Discussions]
