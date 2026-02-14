#!/bin/bash
# VERS SYSTEM 起動スクリプト

# ディレクトリ移動
cd "$(dirname "$0")"

# 古いプロセスを停止
echo "🛑 Stopping old processes..."
pkill -f vers_core

# ビルド (念のため)
echo "🔨 Building..."
cargo build

# 起動
echo "🚀 Starting VERS SYSTEM on port 8080..."
export PORT=8080
export VERS_DASHBOARD_PATH="$(pwd)/vers_dashboard/dist"
./target/debug/vers_core
