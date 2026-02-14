# Exiv Documentation

Documentation for Exiv design, development, and quality management.

## Quick Links

- **[ARCHITECTURE.md](ARCHITECTURE.md)** - Design principles (Manifesto), security framework, plugin communication, Project Oculi
- **[DEVELOPMENT.md](DEVELOPMENT.md)** - Development guardrails (DO NOT list) and refactoring status
- **[CHANGELOG.md](CHANGELOG.md)** - Project change history (Phase 1 → Phase 6)
- **[CODE_QUALITY_AUDIT.md](CODE_QUALITY_AUDIT.md)** - Code quality audit report (Score: 82/100 → 90+/100)

## Current Status (2026-02-13)

**Version:** 0.1.0-alpha.1
**Phase:** 6 Complete ✅
**Code Quality:** 90+/100 (A)
**Design Principles Compliance:** 95+/100 (A)
**Test Coverage:** 45 tests (34 unit + 11 integration)

### Recent Achievements (Phase 6)

1. **Human-in-the-Loop Permissions** - Admin approval workflow for sensitive operations
2. **Rate Limiting** - DoS protection (10 req/s per IP, burst 20)
3. **Audit Logging** - Complete security event trail
4. **Self-Healing Python Bridge** - Auto-restart on crash (max 3 attempts)
5. **Build Optimization** - `EXIV_SKIP_ICON_EMBED=1` for faster development
6. **International Accessibility** - All comments translated to English

## Document Index

| File | Description | Last Updated |
|------|-------------|--------------|
| [ARCHITECTURE.md](ARCHITECTURE.md) | Design principles, security framework, plugin architecture | 2026-02-13 |
| [DEVELOPMENT.md](DEVELOPMENT.md) | Development guidelines, guardrails, coding standards | 2026-02-13 |
| [CODE_QUALITY_AUDIT.md](CODE_QUALITY_AUDIT.md) | Quality audit report and improvement tracking | 2026-02-13 |
| [CHANGELOG.md](CHANGELOG.md) | Comprehensive change history across all phases | 2026-02-13 |

## Plugin Documentation

| Plugin | Location | Description |
|--------|----------|-------------|
| exiv_macros | `exiv_macros/README.md` | Macro optimization guide, icon embedding, CI/CD integration |
| All plugins | `exiv_plugins/*/README.md` | Individual plugin documentation (WIP) |

## Getting Started

```bash
# Clone repository
git clone <repository-url>
cd exiv_project

# Build and run (development)
cargo build
cargo run --package exiv_core

# Build and run (release - optimized)
cargo build --release
cargo run --package exiv_core --release

# Run tests
cargo test

# Fast development builds (skip icon embedding)
export EXIV_SKIP_ICON_EMBED=1
cargo build
```

## API Endpoints

**Core:**
- `GET /api/metrics` - System metrics
- `GET /api/plugins` - Plugin list
- `GET /api/agents` - Agent list
- `GET /api/history` - Event history
- `GET /api/events` - SSE stream

**Admin (Rate Limited):**
- `POST /api/system/shutdown` - Graceful shutdown
- `POST /api/plugins/apply` - Bulk enable/disable
- `POST /api/plugins/:id/config` - Update configuration
- `POST /api/plugins/:id/permissions/grant` - Grant permission

**Human-in-the-Loop (Phase 6):**
- `GET /api/permissions/pending` - Pending permission requests
- `POST /api/permissions/:id/approve` - Approve request
- `POST /api/permissions/:id/deny` - Deny request

## Contributing

See [DEVELOPMENT.md](DEVELOPMENT.md) for coding standards, guardrails, and development workflow.

## License

See project root LICENSE file.
