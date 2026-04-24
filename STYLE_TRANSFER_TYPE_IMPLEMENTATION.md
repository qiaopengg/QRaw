# 风格迁移类型实现指南

## 当前状态

### 已完成

1. ✅ 前端UI：用户可以选择风格迁移类型（人像/风光/城市/通用）
2. ✅ 参数传递：`style_transfer_type`参数已从前端传递到后端
3. ✅ 参数定义：在`StyleTransferRunRequest`和`analyze_style_transfer`函数中已添加参数
4. ✅ 参考图预览：支持所有常见图片格式和RAW格式的预览

### 待实现

后端需要根据`style_transfer_type`参数实现差异化处理逻辑。

## 实现建议

根据文档`rapid_raw_分析式风格迁移技术架构_v_4.md`的指导，不同类型应该有不同的处理策略：

### 1. 人像（portrait）

**优化重点**：

- 加强肤色保护（增加`skin_protect_strength`）
- 优先识别和保护人物主体区域
- 对背景采用更保守的迁移策略
- 高光保护更激进，避免肤色过曝

**建议实现位置**：

```rust
// 在 analyze_style_transfer 函数中
let (adjusted_style_strength, adjusted_highlight_guard, adjusted_skin_protect) =
    match style_transfer_type.as_deref() {
        Some("portrait") => (
            style_strength.map(|v| v * 0.95), // 稍微降低整体强度
            highlight_guard_strength.map(|v| v * 1.2), // 增强高光保护
            skin_protect_strength.map(|v| v * 1.3), // 显著增强肤色保护
        ),
        Some("landscape") => (
            style_strength.map(|v| v * 1.05), // 稍微增加整体强度
            highlight_guard_strength.map(|v| v * 0.95), // 适度降低高光保护
            skin_protect_strength, // 保持默认
        ),
        Some("urban") => (
            style_strength.map(|v| v * 1.1), // 增加整体强度
            highlight_guard_strength.map(|v| v * 1.15), // 增强高光保护（夜景）
            skin_protect_strength, // 保持默认
        ),
        _ => (style_strength, highlight_guard_strength, skin_protect_strength),
    };
```

### 2. 风光（landscape）

**优化重点**：

- 优化天空区域的色彩迁移
- 加强植被的绿色处理
- 对自然场景采用更激进的迁移策略
- 允许更大的色彩变化范围

**建议实现**：

- 在`build_style_transfer_context`中添加场景类型参数
- 在特征提取时，对天空和植被区域给予更高权重
- 在`run_algorithm_pipeline`中，根据类型调整HSL映射策略

### 3. 城市（urban）

**优化重点**：

- 优化建筑线条和结构
- 加强夜景的高光处理
- 对霓虹灯等高饱和度区域特殊处理
- 保护建筑物的细节

**建议实现**：

- 增强高光保护，避免霓虹灯过曝
- 对高饱和度区域采用更保守的映射
- 在暗部保留更多细节

### 4. 通用（general）

**优化重点**：

- 平衡各类场景的处理
- 使用默认的参数配置
- 不做特殊的区域优化

## 实现步骤

### 步骤1：在`analyze_style_transfer`中调整参数

```rust
// 在函数开始处，根据类型调整tuning参数
let tuning = match style_transfer_type.as_deref() {
    Some("portrait") => StyleTransferTuning {
        style_strength: style_strength.unwrap_or(1.0) * 0.95,
        highlight_guard_strength: highlight_guard_strength.unwrap_or(1.0) * 1.2,
        skin_protect_strength: skin_protect_strength.unwrap_or(1.0) * 1.3,
    },
    Some("landscape") => StyleTransferTuning {
        style_strength: style_strength.unwrap_or(1.0) * 1.05,
        highlight_guard_strength: highlight_guard_strength.unwrap_or(1.0) * 0.95,
        skin_protect_strength: skin_protect_strength.unwrap_or(1.0),
    },
    Some("urban") => StyleTransferTuning {
        style_strength: style_strength.unwrap_or(1.0) * 1.1,
        highlight_guard_strength: highlight_guard_strength.unwrap_or(1.0) * 1.15,
        skin_protect_strength: skin_protect_strength.unwrap_or(1.0),
    },
    _ => StyleTransferTuning::from_options(
        style_strength,
        highlight_guard_strength,
        skin_protect_strength,
    ),
};
```

### 步骤2：在上下文构建中传递类型信息

```rust
// 修改 build_style_transfer_context 函数签名
fn build_style_transfer_context(
    ref_img: DynamicImage,
    cur_img: DynamicImage,
    aux_imgs: Vec<DynamicImage>,
    current_adjustments: &Value,
    tuning: StyleTransferTuning,
    enable_expert_preset: bool,
    style_backbone: Option<Arc<StyleTransferBackbone>>,
    style_transfer_type: Option<&str>, // 新增参数
) -> StyleTransferContext {
    // ... 在上下文中记录类型信息
}
```

### 步骤3：在算法管道中使用类型信息

```rust
// 在 run_algorithm_pipeline 中根据类型调整处理策略
fn run_algorithm_pipeline(
    ctx: &StyleTransferContext,
    current_adjustments: &Value,
    options: StyleTransferAlgoOptions,
) -> AlgorithmPipelineResult {
    // 根据 ctx.style_transfer_type 调整处理逻辑
    match ctx.style_transfer_type {
        Some("portrait") => {
            // 人像特殊处理
            // - 增强肤色区域的权重
            // - 对背景采用更保守的映射
        },
        Some("landscape") => {
            // 风光特殊处理
            // - 增强天空和植被的映射
            // - 允许更大的色彩变化
        },
        Some("urban") => {
            // 城市特殊处理
            // - 增强高光保护
            // - 对高饱和度区域特殊处理
        },
        _ => {
            // 通用处理
        }
    }
}
```

## 测试建议

### 1. 人像测试

- 导入人像参考图
- 选择"人像"类型
- 验证：
  - 肤色是否得到更好的保护
  - 高光是否不会过曝
  - 背景变化是否更保守

### 2. 风光测试

- 导入风光参考图
- 选择"风光"类型
- 验证：
  - 天空颜色是否更接近参考图
  - 植被颜色是否更自然
  - 整体色彩变化是否更明显

### 3. 城市测试

- 导入城市夜景参考图
- 选择"城市"类型
- 验证：
  - 霓虹灯等高光是否得到保护
  - 建筑细节是否保留
  - 暗部是否有足够细节

### 4. 对比测试

- 使用相同参考图
- 分别测试不同类型
- 对比输出差异

## 注意事项

1. **谨慎修改**：当前的功能逻辑是经过多次迭代的结果，修改时要确保不影响主流程
2. **渐进式实现**：建议先实现参数调整，验证效果后再实现更复杂的区域处理
3. **保持兼容**：确保不选择类型时（或选择"通用"）的行为与当前版本一致
4. **添加日志**：在处理调试信息中显示选择的类型和应用的调整

## 当前修改的文件

### 前端

1. `src/components/panel/right/chat/styleTransfer/StyleTransferReferenceSelectionCard.tsx`
   - 添加了RAW格式支持：dng, nef, cr2, cr3, arw, raf, orf, rw2
   - 修复了预览功能

### 后端

1. `src-tauri/src/style_transfer_runtime.rs`
   - 在`run_style_transfer`中传递`style_transfer_type`参数

2. `src-tauri/src/style_transfer.rs`
   - 在`analyze_style_transfer`函数签名中添加`style_transfer_type`参数
   - **待实现**：在函数内部使用该参数调整处理逻辑

## 下一步

1. 在`analyze_style_transfer`函数中实现参数调整逻辑
2. 在处理调试信息中显示选择的类型
3. 测试不同类型的效果差异
4. 根据测试结果微调参数
