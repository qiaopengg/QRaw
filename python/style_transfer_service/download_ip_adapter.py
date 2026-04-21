#!/usr/bin/env python3
"""
手动下载 IP-Adapter 模型（使用国内镜像）
"""
import os
from huggingface_hub import hf_hub_download

# 配置国内镜像
os.environ['HF_ENDPOINT'] = 'https://hf-mirror.com'

print("=" * 60)
print("下载 IP-Adapter 模型")
print("=" * 60)
print(f"使用镜像: {os.environ['HF_ENDPOINT']}")
print()

# 下载模型
repo_id = "h94/IP-Adapter"
filename = "sdxl_models/ip-adapter-plus_sdxl_vit-h.safetensors"

print(f"仓库: {repo_id}")
print(f"文件: {filename}")
print()
print("开始下载...")
print()

try:
    file_path = hf_hub_download(
        repo_id=repo_id,
        filename=filename,
        resume_download=True
    )
    print()
    print("✅ 下载完成！")
    print(f"文件位置: {file_path}")
except Exception as e:
    print()
    print(f"❌ 下载失败: {e}")
    print()
    print("请检查网络连接或尝试使用代理")
