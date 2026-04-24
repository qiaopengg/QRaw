# AI风格迁移滑块标签对齐修复

## 问题描述

AI风格迁移会话框中出现的调色滑块名称与项目基建的调色滑块功能不一致，例如：

- 系统基建使用"亮度"，但会话框显示"曝光"
- 标签名称不统一，影响用户体验

## 问题原因

1. **后端硬编码标签**：Rust代码中直接硬编码了中文标签字符串
2. **前端未使用i18n**：前端直接显示后端返回的label，没有通过i18n系统翻译
3. **缺少翻译键**：部分参数（如`vignetteAmount`）在i18n文件中没有定义

## 修复方案

### 1. 前端修改

**文件**: `src/components/panel/right/chat/styleTransfer/StyleTransferSuggestionsCard.tsx`

**修改前**:

```tsx
<Slider
  label={suggestion.label}
  // ...
/>
```

**修改后**:

```tsx
<Slider
  label={t(`adjustments.${suggestion.key}`) || suggestion.label}
  // ...
/>
```

**说明**:

- 优先使用i18n系统的翻译：`t(\`adjustments.${suggestion.key}\`)`
- 如果翻译不存在，回退到后端返回的label
- 这样确保标签与项目基建的调色参数严格对齐

### 2. i18n文件更新

**文件**: `src/i18n/locales/zh-CN.json` 和 `src/i18n/locales/en.json`

**添加缺失的翻译**:

```json
{
  "adjustments": {
    "vignetteAmount": "暗角" // 中文
    // "vignetteAmount": "Vignette"  // 英文
  }
}
```

## 参数键名对照表

| 参数键 (key)   | 中文标签   | 英文标签    | i18n键                     |
| -------------- | ---------- | ----------- | -------------------------- |
| exposure       | 曝光       | Exposure    | adjustments.exposure       |
| brightness     | 亮度       | Brightness  | adjustments.brightness     |
| contrast       | 对比度     | Contrast    | adjustments.contrast       |
| highlights     | 高光       | Highlights  | adjustments.highlights     |
| shadows        | 阴影       | Shadows     | adjustments.shadows        |
| whites         | 白色       | Whites      | adjustments.whites         |
| blacks         | 黑色       | Blacks      | adjustments.blacks         |
| temperature    | 色温       | Temperature | adjustments.temperature    |
| tint           | 色调偏移   | Tint        | adjustments.tint           |
| saturation     | 饱和度     | Saturation  | adjustments.saturation     |
| vibrance       | 自然饱和度 | Vibrance    | adjustments.vibrance       |
| clarity        | 清晰度     | Clarity     | adjustments.clarity        |
| vignetteAmount | 暗角       | Vignette    | adjustments.vignetteAmount |

## 后端参数键使用情况

根据`src-tauri/src/style_transfer.rs`中的`map_features_to_adjustments`函数，后端生成的参数键包括：

1. **基础调色**:
   - exposure (曝光)
   - contrast (对比度)
   - highlights (高光)
   - shadows (阴影)
   - whites (白色)
   - blacks (黑色)

2. **色彩调整**:
   - temperature (色温)
   - tint (色调偏移)
   - saturation (饱和度)
   - vibrance (自然饱和度)

3. **细节和效果**:
   - clarity (清晰度)
   - vignetteAmount (暗角)

4. **高级调整**:
   - curves (曲线) - 使用complex_value
   - hsl (颜色混合器) - 使用complex_value

## 修复后的效果

### 修复前

- 后端返回：`{ key: "exposure", label: "曝光", value: 0.5 }`
- 前端显示：**曝光** (直接使用后端的label)
- 问题：如果后端label与基建不一致，会造成混乱

### 修复后

- 后端返回：`{ key: "exposure", label: "曝光", value: 0.5 }`
- 前端显示：**曝光** (通过`t('adjustments.exposure')`获取)
- 优势：
  1. 标签与项目基建严格对齐
  2. 支持多语言切换
  3. 统一的标签管理

## 验证方法

1. **检查标签一致性**:
   - 打开AI风格迁移功能
   - 导入参考图并运行分析
   - 检查会话框中的滑块标签
   - 对比右侧调色面板的标签
   - 确认标签完全一致

2. **检查多语言支持**:
   - 切换到英文界面
   - 运行风格迁移
   - 确认所有标签都正确翻译为英文

3. **检查所有参数**:
   - 测试不同的参考图和当前图组合
   - 确保所有可能生成的参数都有正确的标签
   - 特别注意`vignetteAmount`等新添加的参数

## 注意事项

1. **后端label作为回退**:
   - 如果i18n中没有对应的键，会使用后端返回的label
   - 这确保了即使有新参数，也不会显示为空

2. **参数键必须匹配**:
   - 后端生成的`key`必须与前端`Adjustments`接口中的字段名匹配
   - 后端生成的`key`必须与i18n中的键名匹配（`adjustments.${key}`）

3. **复杂值参数**:
   - `curves`和`hsl`等使用`complex_value`的参数有特殊处理
   - 这些参数的标签也应该通过i18n系统获取

## 相关文件

- `src/components/panel/right/chat/styleTransfer/StyleTransferSuggestionsCard.tsx` - 前端滑块显示组件
- `src/i18n/locales/zh-CN.json` - 中文翻译
- `src/i18n/locales/en.json` - 英文翻译
- `src/utils/adjustments.tsx` - 调色参数定义
- `src-tauri/src/style_transfer.rs` - 后端参数生成逻辑

## 后续优化建议

1. **移除后端硬编码标签**:
   - 后端可以只返回`key`，不返回`label`
   - 所有标签由前端i18n系统统一管理
   - 这样可以减少后端代码的维护负担

2. **添加参数验证**:
   - 在开发环境中，验证所有后端返回的`key`都有对应的i18n翻译
   - 如果缺少翻译，在控制台输出警告

3. **统一标签管理**:
   - 创建一个中心化的参数定义文件
   - 包含所有参数的键名、类型、范围、默认值等
   - 前后端共享这个定义

## 总结

本次修复确保了AI风格迁移会话框中的滑块标签与项目基建的调色滑块功能严格对齐，通过使用i18n系统统一管理标签，提高了代码的可维护性和用户体验的一致性。
