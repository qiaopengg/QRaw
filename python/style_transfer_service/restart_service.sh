#!/bin/bash

set -euo pipefail

cd "$(dirname "$0")"

SERVICE_PORT="${QRAW_STYLE_TRANSFER_PORT:-7860}"
SERVICE_ENTRY="${QRAW_STYLE_TRANSFER_ENTRY:-app.py}"

echo "=========================================="
echo "重启 QRaw 风格迁移服务"
echo "=========================================="
echo ""

# 查找并停止现有服务
echo "正在查找现有服务进程..."
PID="$(lsof -tiTCP:${SERVICE_PORT} -sTCP:LISTEN 2>/dev/null | head -n 1 || true)"

if [ -n "$PID" ]; then
    echo "找到进程 PID: $PID"
    echo "正在停止..."
    kill "$PID"
    sleep 2
    echo "✓ 已停止"
else
    echo "没有找到运行中的服务"
fi

echo ""
echo "启动服务（调试模式）..."
echo "=========================================="
echo ""

export QRAW_DEBUG="1"
export QRAW_STYLE_TRANSFER_ENTRY="$SERVICE_ENTRY"
bash start_service.sh
