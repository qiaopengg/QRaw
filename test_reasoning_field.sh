#!/bin/bash

echo "=== qwen3.6:27b Reasoning 字段测试 ==="
echo ""

echo "测试 1: 验证 qwen3.6:27b 的响应格式"
echo "----------------------------------------"
curl -s -X POST http://localhost:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen3.6:27b",
    "messages": [{"role": "user", "content": "简短回答：1+1=?"}],
    "stream": true
  }' | head -5 | grep -o '"reasoning":"[^"]*"' | head -3

echo ""
echo ""

echo "测试 2: 验证响应中是否有 reasoning 字段"
echo "----------------------------------------"
RESPONSE=$(curl -s -X POST http://localhost:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen3.6:27b",
    "messages": [{"role": "user", "content": "你好"}],
    "stream": true
  }' | head -10)

if echo "$RESPONSE" | grep -q '"reasoning"'; then
    echo "✅ 发现 reasoning 字段"
else
    echo "❌ 未发现 reasoning 字段"
fi

if echo "$RESPONSE" | grep -q '"content"'; then
    echo "✅ 发现 content 字段"
else
    echo "❌ 未发现 content 字段"
fi

echo ""
echo ""

echo "测试 3: 对比 qwen3.5:9b 的响应格式"
echo "----------------------------------------"
RESPONSE_35=$(curl -s -X POST http://localhost:11434/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "qwen3.5:9b",
    "messages": [{"role": "user", "content": "你好"}],
    "stream": true
  }' | head -10)

if echo "$RESPONSE_35" | grep -q '"reasoning"'; then
    echo "⚠️  qwen3.5:9b 有 reasoning 字段（不常见）"
else
    echo "✅ qwen3.5:9b 没有 reasoning 字段（正常）"
fi

if echo "$RESPONSE_35" | grep -q '"content"'; then
    echo "✅ qwen3.5:9b 有 content 字段（正常）"
else
    echo "❌ qwen3.5:9b 没有 content 字段（异常）"
fi

echo ""
echo ""

echo "=== 修复验证 ==="
echo "修复内容："
echo "1. ✅ 同时处理 reasoning 和 content 字段"
echo "2. ✅ 优先使用 reasoning（qwen3.6:27b）"
echo "3. ✅ 向后兼容 content（旧模型）"
echo ""
echo "测试方法："
echo "1. 启动应用程序"
echo "2. 打开一张图片"
echo "3. 导入参考图进行风格迁移"
echo "4. 选择 qwen3.6:27b 模型"
echo "5. 观察是否还会出现 'error decoding response body' 错误"
echo ""
echo "预期结果："
echo "✅ 不会出现 'error decoding response body' 错误"
echo "✅ VLM 功能正常工作"
echo "✅ 能够看到思考过程（reasoning）"
echo "✅ 能够获得最终结果（content）"
