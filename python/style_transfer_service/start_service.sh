#!/bin/bash

set -euo pipefail

cd "$(dirname "$0")"

# 配置 Hugging Face 国内镜像
export HF_ENDPOINT="${HF_ENDPOINT:-https://hf-mirror.com}"
export QRAW_DEBUG="${QRAW_DEBUG:-1}"
export PYTHONUNBUFFERED=1
export QRAW_IP_ADAPTER_MODEL="${QRAW_IP_ADAPTER_MODEL:-$(pwd)/models/ip_adapter}"
export QRAW_IP_ADAPTER_WEIGHT="${QRAW_IP_ADAPTER_WEIGHT:-ip-adapter-plus_sdxl_vit-h.safetensors}"
export QRAW_STYLE_TRANSFER_HOST="${QRAW_STYLE_TRANSFER_HOST:-127.0.0.1}"
export QRAW_STYLE_TRANSFER_PORT="${QRAW_STYLE_TRANSFER_PORT:-7860}"
SERVICE_ENTRY="${QRAW_STYLE_TRANSFER_ENTRY:-app.py}"

IP_ADAPTER_RELATIVE_PATH="sdxl_models/${QRAW_IP_ADAPTER_WEIGHT}"
IP_ADAPTER_TARGET_PATH="${QRAW_IP_ADAPTER_MODEL}/${IP_ADAPTER_RELATIVE_PATH}"
IP_ADAPTER_DOWNLOAD_URL="${HF_ENDPOINT}/h94/IP-Adapter/resolve/main/${IP_ADAPTER_RELATIVE_PATH}"
IP_ADAPTER_ENCODER_DIR="${QRAW_IP_ADAPTER_MODEL}/models/image_encoder"
HEALTH_URL="http://${QRAW_STYLE_TRANSFER_HOST}:${QRAW_STYLE_TRANSFER_PORT}/health"
IP_ADAPTER_ENCODER_FILES=(
  "config.json"
  "model.safetensors"
)

ensure_ip_adapter_model() {
  if [ -f "$IP_ADAPTER_TARGET_PATH" ]; then
    echo "检测到本地 IP-Adapter 模型: $IP_ADAPTER_TARGET_PATH"
  else
    echo "未检测到本地 IP-Adapter 模型，开始下载..."
    mkdir -p "$(dirname "$IP_ADAPTER_TARGET_PATH")"
    curl -L --fail --retry 3 --connect-timeout 15 "$IP_ADAPTER_DOWNLOAD_URL" -o "$IP_ADAPTER_TARGET_PATH"
    echo "IP-Adapter 模型已下载完成: $IP_ADAPTER_TARGET_PATH"
  fi

  mkdir -p "$IP_ADAPTER_ENCODER_DIR"
  for file_name in "${IP_ADAPTER_ENCODER_FILES[@]}"; do
    local target_path="${IP_ADAPTER_ENCODER_DIR}/${file_name}"
    local source_url="${HF_ENDPOINT}/h94/IP-Adapter/resolve/main/models/image_encoder/${file_name}"
    if [ -f "$target_path" ]; then
      echo "检测到本地 image encoder 文件: $target_path"
      continue
    fi
    echo "下载 IP-Adapter image encoder 文件: ${file_name}"
    curl -L --fail --retry 3 --connect-timeout 15 "$source_url" -o "$target_path"
  done
}

reuse_existing_service_if_ready() {
  local port_owner
  port_owner="$(lsof -tiTCP:${QRAW_STYLE_TRANSFER_PORT} -sTCP:LISTEN 2>/dev/null | head -n 1 || true)"

  if [ -z "$port_owner" ]; then
    return 1
  fi

  echo "检测到 ${QRAW_STYLE_TRANSFER_HOST}:${QRAW_STYLE_TRANSFER_PORT} 已有监听进程: PID ${port_owner}"

  local health_response
  health_response="$(curl -fsS --connect-timeout 2 --max-time 5 "$HEALTH_URL" 2>/dev/null || true)"
  if [ -n "$health_response" ]; then
    echo "检测到现有 QRaw 风格迁移服务，复用该实例: $health_response"
    return 0
  fi

  local owner_command
  owner_command="$(ps -p "$port_owner" -o command= 2>/dev/null || true)"
  if [[ "$owner_command" == *"app.py"* || "$owner_command" == *"app_fixed.py"* ]]; then
    echo "检测到旧的 QRaw 风格迁移实例健康异常，准备清理后重启: PID ${port_owner}"
    kill "$port_owner" 2>/dev/null || true
    sleep 1
    return 1
  fi

  echo "端口 ${QRAW_STYLE_TRANSFER_PORT} 已被占用，但健康检查未通过，请先释放端口后重试。"
  return 2
}

# 启动服务
echo "使用 Hugging Face 镜像: $HF_ENDPOINT"
ensure_ip_adapter_model
if reuse_existing_service_if_ready; then
  exit 0
fi
echo "启动 QRaw 风格迁移服务: $(pwd)/${SERVICE_ENTRY}"
python3 "$SERVICE_ENTRY"
