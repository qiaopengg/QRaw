#!/bin/bash
# 清空风格迁移缓存

CACHE_DIR="/var/folders/mf/32brj6rn5qdgwddgl88f4brw0000gn/T/qraw-style-transfer"

echo "清空风格迁移缓存..."
echo "目录: $CACHE_DIR"

if [ -d "$CACHE_DIR" ]; then
    rm -rf "$CACHE_DIR"/*
    echo "✅ 缓存已清空"
else
    echo "⚠️  缓存目录不存在"
fi

echo ""
echo "现在可以重新测试风格迁移了"
