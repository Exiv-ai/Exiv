#!/bin/bash
# Exiv System - Startup Script (Guardian Managed)
set -e
cd "$(dirname "$0")"

# Load environment variables from .env file if it exists
if [ -f .env ]; then
    set -a
    source .env
    set +a
fi

# Configuration (with defaults; override via .env or environment)
export PORT="${PORT:-8081}"
export EXIV_DASHBOARD_PATH="${EXIV_DASHBOARD_PATH:-$(pwd)/exiv_dashboard/dist}"
export DATABASE_URL="${DATABASE_URL:-sqlite:$(pwd)/exiv_memories.db}"
export RUST_LOG="${RUST_LOG:-info}"

echo "🛑 Ensuring clean state for dev environment..."
# 安定版(8080)以外の開発用プロセスを掃除
pkill -9 -f exiv_guardian || true
pkill -9 -f target/debug/exiv_core || true
rm -f .maintenance

echo "🔨 Building Exiv Core..."
cargo build

# ガーディアンが監視する実行ファイル
TARGET_BIN="./target/debug/exiv_system"

echo "🚀 Launching Exiv via Guardian on port ${PORT}..."
# Guardianはカレントディレクトリのバイナリを監視・実行する
# Guardian monitors the binary and auto-restarts on crash or update.

nohup ./exiv_guardian > kernel.log 2>&1 &
echo "✅ Exiv System is now under Guardian protection (Port: ${PORT})."
echo "📡 You can use 'POST /api/system/shutdown' to trigger a safe maintenance restart."
