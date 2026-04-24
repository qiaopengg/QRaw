#!/bin/bash

echo "=== QRaw 应用重启脚本 ==="
echo ""

# 查找正在运行的 QRaw 进程
QRAW_PID=$(ps aux | grep "target/debug/QRaw" | grep -v grep | awk '{print $2}')

if [ -n "$QRAW_PID" ]; then
    echo "发现正在运行的 QRaw 进程（PID: $QRAW_PID）"
    echo "正在停止..."
    kill $QRAW_PID
    sleep 2
    echo "✅ 已停止"
else
    echo "未发现正在运行的 QRaw 进程"
fi

echo ""
echo "=== 修复已完成 ==="
echo "1. ✅ 修改了 llm_chat.rs"
echo "2. ✅ 修改了 style_transfer.rs"
echo "3. ✅ 重新编译了代码"
echo ""
echo "=== 下一步 ==="
echo "请手动重启开发服务器："
echo ""
echo "  npm run tauri dev"
echo ""
echo "或者如果你使用的是其他命令，请重新运行该命令。"
echo ""
echo "=== 验证修复 ==="
echo "重启后，请测试："
echo "1. 打开一张图片"
echo "2. 导入参考图进行风格迁移"
echo "3. 选择 qwen3.6:27b 模型"
echo "4. 观察是否还会出现 'error decoding response body' 错误"
echo ""
echo "预期结果："
echo "✅ 不会出现错误"
echo "✅ VLM 功能正常工作"
echo "✅ 能够看到思考过程"
