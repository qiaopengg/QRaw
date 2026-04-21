"""
紧急修复：禁用所有后处理，直接输出 Pipeline 结果
用于诊断问题是在 Pipeline 还是后处理
"""

# 在 style_transfer() 函数中，找到这部分代码并替换：

# 原代码（有问题）：
"""
    # 🆕 后处理步骤 1: 色彩对齐（文档第 7.2 节）
    if req.enable_color_alignment and HAS_COLOR_ALIGNMENT:
        _print_progress(92, "应用色彩对齐...", task_id)
        ...
    
    # 🆕 后处理步骤 2: RAW 融合（文档第 7.3 节）
    if req.enable_raw_fusion and req.preserve_raw_tone_curve and HAS_RAW_FUSION:
        _print_progress(94, "应用 RAW 融合...", task_id)
        ...
"""

# 替换为（紧急修复）：
"""
    # 🔧 紧急修复：跳过所有后处理，直接保存 Pipeline 输出
    _print_progress(92, "跳过后处理（紧急修复模式）...", task_id)
    print("[EMERGENCY_FIX] Skipping all post-processing")
    print(f"[EMERGENCY_FIX] Result shape: {result.shape}, dtype: {result.dtype}")
    print(f"[EMERGENCY_FIX] Result range: [{result.min():.2f}, {result.max():.2f}]")
    print(f"[EMERGENCY_FIX] Result mean: {result.mean():.2f}")
    print(f"[EMERGENCY_FIX] RGB channels: R={result[:,:,0].mean():.2f}, G={result[:,:,1].mean():.2f}, B={result[:,:,2].mean():.2f}")
"""

# 具体修改步骤：
# 1. 打开 app.py
# 2. 找到第 880 行左右的 "# 🆕 后处理步骤 1: 色彩对齐"
# 3. 注释掉整个色彩对齐和 RAW 融合的代码块
# 4. 添加上面的调试输出
# 5. 重启服务测试
