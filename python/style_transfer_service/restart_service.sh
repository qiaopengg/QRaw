#!/bin/bash

echo "=========================================="
echo "重启风格迁移服务（紧急修复模式）"
echo "=========================================="
echo ""
echo "已禁用所有后处理（色彩对齐 + RAW 融合）"
echo "直接输出 SDXL Pipeline 结果"
echo ""
echo "这将帮助诊断问题是在："
echo "  1. SDXL Pipeline 本身"
echo "  2. 还是后处理步骤"
echo ""
echo "=========================================="
echo ""

# 查找并停止现有服务
echo "正在查找现有服务进程..."
PID=$(ps aux | grep "python.*app.py" | grep -v grep | awk '{print $2}')

if [ ! -z "$PID" ]; then
    echo "找到进程 PID: $PID"
    echo "正在停止..."
    kill $PID
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
python3 app.py
