#!/bin/bash

# 配置 Hugging Face 国内镜像
export HF_ENDPOINT="https://hf-mirror.com"
export QRAW_DEBUG="1"

# 启动服务
echo "使用 Hugging Face 镜像: $HF_ENDPOINT"
python3 app.py
