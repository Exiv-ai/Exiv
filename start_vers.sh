#!/bin/bash
# VERS SYSTEM - Next Generation Startup Script (Guardian Managed)
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
export VERS_DASHBOARD_PATH="${VERS_DASHBOARD_PATH:-$(pwd)/vers_dashboard/dist}"
export DATABASE_URL="${DATABASE_URL:-sqlite:$(pwd)/vers_memories.db}"
export RUST_LOG="${RUST_LOG:-info}"

echo "🛑 Ensuring clean state for dev environment..."
# 安定版(8080)以外の開発用プロセスを掃除
pkill -9 -f vers_guardian || true
pkill -9 -f target/debug/vers_core || true
rm -f .maintenance

echo "🔨 Building VERS Core..."
cargo build

# ガーディアンが監視する実行ファイル
TARGET_BIN="./target/debug/vers_system"

echo "🚀 Launching VERS via Guardian on port ${PORT}..."
# Guardianはカレントディレクトリのバイナリを監視・実行する
# 今回は guardian.rs の仕様に合わせ、AI Karin パスとして vers_core を認識させる
# (バイナリ更新監視機能を活用するため)

nohup ./vers_guardian > kernel.log 2>&1 &
echo "✅ VERS System is now under Guardian protection (Port: ${PORT})."
echo "📡 You can use 'POST /api/system/shutdown' to trigger a safe maintenance restart."
