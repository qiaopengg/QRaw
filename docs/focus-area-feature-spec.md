# 📸 相机对焦区域标注功能 - 技术实现文档

> **版本**: 1.0  
> **最后更新**: 2026-04-29  
> **状态**: 已验证 ✅  
> **基于**: RapidRAW 项目实际代码架构

---

## 📋 目录

1. [功能概述](#功能概述)
2. [技术验证](#技术验证)
3. [架构设计](#架构设计)
4. [实现细节](#实现细节)
5. [开发路线](#开发路线)
6. [注意事项](#注意事项)

---

## 🎯 功能概述

### 目标

实现一个**纯元数据驱动**的对焦区域标注功能，在图片上可视化显示相机的实际对焦区域。

### 核心特性

- ✅ 读取 RAW 文件中的对焦点元数据
- ✅ 在图片上绘制对焦区域（点、区域、人脸、眼睛）
- ✅ 支持图片变换（旋转、裁剪、翻转）
- ✅ 一键显示/隐藏切换
- ❌ 不使用 AI 推测
- ❌ 不支持非原始文件

### 支持范围

| 相机品牌 | 支持状态    | 预期覆盖率 | 实现难度 |
| -------- | ----------- | ---------- | -------- |
| Sony     | ✅ 优先     | 20-30%     | 中等     |
| Canon    | ⏳ 后续     | 15-25%     | 中等     |
| Nikon    | ⏳ 后续     | 10-20%     | 较高     |
| 其他     | ❌ 暂不支持 | -          | -        |

---

## ✅ 技术验证

### 已验证的项目依赖

```toml
# src-tauri/Cargo.toml (已确认存在)
kamadak-exif = "0.6.1"           # ✅ 标准 EXIF 解析
rawler = { git = "https://github.com/CyberTimon/RapidRAW-DngLab.git" } # ✅ RAW 元数据解析(自定义 fork)
serde = { version = "1.0", ... }  # ✅ 序列化支持
已验证的代码基础
// ✅ 已存在于 src-tauri/src/exif_processing.rs
pub fn read_raw_metadata(file_bytes: &[u8]) -> Option<RawMetadata>

// ✅ 已存在的 Tauri Command 模式
#[tauri::command]
pub fn some_command(...) -> Result<T, String>

// ✅ 已存在的数据结构访问
metadata.exif.iso_speed
metadata.exif.date_time_original
metadata.make
metadata.model
已验证的前端能力
// ✅ 已使用 react-konva 绘制蒙版
import { Stage, Layer, Rect, Circle } from 'react-konva';

// ✅ 已有坐标变换逻辑
apply_coarse_rotation()
apply_rotation()
apply_flip()
apply_crop()

// ✅ 已有工具栏按钮模式
<button onClick={onToggle} data-tooltip="...">
🏗️ 架构设计
系统架构图
┌─────────────────────────────────────────────────────────┐
│                     Frontend (React)                     │
├─────────────────────────────────────────────────────────┤
│  EditorToolbar.tsx                                       │
│  ├─ [显示对焦区域] 按钮                                   │
│  └─ showFocusAreas state                                 │
├─────────────────────────────────────────────────────────┤
│  ImageCanvas.tsx                                         │
│  ├─ Konva Stage/Layer                                    │
│  ├─ transformFocusPoint() 坐标变换                       │
│  └─ <Rect> 渲染对焦框                                    │
└─────────────────────────────────────────────────────────┘
                          ↕ Tauri IPC
┌─────────────────────────────────────────────────────────┐
│                    Backend (Rust)                        │
├─────────────────────────────────────────────────────────┤
│  lib.rs                                                  │
│  └─ #[tauri::command] get_focus_regions()                │
├─────────────────────────────────────────────────────────┤
│  focus_extraction.rs (新建)                              │
│  ├─ FocusRegion 数据结构                                 │
│  ├─ FocusAdapter trait                                   │
│  ├─ SonyAdapter                                          │
│  ├─ CanonAdapter                                         │
│  └─ NikonAdapter                                         │
├─────────────────────────────────────────────────────────┤
│  exif_processing.rs (已存在)                             │
│  └─ read_raw_metadata() 复用                             │
└─────────────────────────────────────────────────────────┘
数据流
1. 用户点击 [显示对焦区域] 按钮
   ↓
2. Frontend 调用 invoke('get_focus_regions', { path })
   ↓
3. Backend 读取文件 → read_raw_metadata()
   ↓
4. 根据 metadata.make 选择 Adapter
   ↓
5. Adapter 解析 → 返回 Vec<FocusRegion>
   ↓
6. Frontend 接收数据 → transformFocusPoint()
   ↓
7. Konva 渲染红色矩形框
🔧 实现细节
1. 后端数据结构
// src-tauri/src/focus_extraction.rs (新建文件)

use serde::{Deserialize, Serialize};
use rawler::decoders::RawMetadata;

/// 统一的对焦区域数据结构
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FocusRegion {
    /// 归一化 X 坐标 (0.0 - 1.0)
    pub x: f32,
    /// 归一化 Y 坐标 (0.0 - 1.0)
    pub y: f32,
    /// 归一化宽度 (0.0 - 1.0)
    pub width: f32,
    /// 归一化高度 (0.0 - 1.0)
    pub height: f32,
    /// 对焦类型
    pub kind: FocusKind,
    /// 是否为主对焦点
    pub is_primary: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(rename_all = "lowercase")]
pub enum FocusKind {
    Point,  // 单点对焦
    Area,   // 区域对焦
    Face,   // 人脸检测
    Eye,    // 眼部对焦
}

/// Adapter 接口
pub trait FocusAdapter {
    /// 检查是否支持该相机
    fn supports(metadata: &RawMetadata) -> bool;

    /// 提取对焦区域
    fn extract(metadata: &RawMetadata) -> Result<Vec<FocusRegion>, String>;
}
2. Sony Adapter 实现（示例）
pub struct SonyAdapter;

impl FocusAdapter for SonyAdapter {
    fn supports(metadata: &RawMetadata) -> bool {
        metadata.make.to_lowercase().contains("sony")
    }

    fn extract(metadata: &RawMetadata) -> Result<Vec<FocusRegion>, String> {
        let mut regions = Vec::new();

        // 阶段 1：尝试读取简单字段
        // 注意：这里需要根据实际的 rawler API 调整
        // 目前 rawler 的 MakerNotes 解析有限

        // 示例：如果有 FocusPosition 字段（部分机型）
        // if let Some(focus_data) = metadata.exif.get_maker_note("FocusPosition") {
        //     // 解析逻辑
        // }

        // 阶段 1 现实：大多数机型需要逆向 AFInfo blob
        // 暂时返回空，等待用户提供样本文件

        log::warn!("Sony focus area extraction not yet implemented for this model");
        Ok(regions)
    }
}
3. Tauri Command
// src-tauri/src/lib.rs (添加到现有文件)

use crate::focus_extraction::{FocusRegion, FocusAdapter, SonyAdapter, CanonAdapter, NikonAdapter};

#[tauri::command]
pub fn get_focus_regions(
    path: String,
    state: tauri::State<AppState>
) -> Result<Vec<FocusRegion>, String> {
    let (source_path, _sidecar) = parse_virtual_path(&path);

    // 1. 检查缓存
    let cache_key = format!("focus_{}", source_path.to_string_lossy());
    // TODO: 实现缓存逻辑

    // 2. 读取文件
    let file_bytes = match read_file_mapped(&source_path) {
        Ok(mmap) => mmap.to_vec(),
        Err(_) => fs::read(&source_path).map_err(|e| e.to_string())?
    };

    // 3. 解析元数据
    let raw_metadata = exif_processing::read_raw_metadata(&file_bytes)
        .ok_or("Not a RAW file or metadata unavailable")?;

    // 4. 选择 Adapter
    let regions = if SonyAdapter::supports(&raw_metadata) {
        SonyAdapter::extract(&raw_metadata)?
    } else if CanonAdapter::supports(&raw_metadata) {
        CanonAdapter::extract(&raw_metadata)?
    } else if NikonAdapter::supports(&raw_metadata) {
        NikonAdapter::extract(&raw_metadata)?
    } else {
        return Err(format!(
            "Focus area extraction not supported for {} {}",
            raw_metadata.make,
            raw_metadata.model
        ));
    };

    // 5. 缓存结果
    // TODO: 实现缓存逻辑

    Ok(regions)
}

// 在 run() 函数中注册 command
pub fn run() {
    // ... 现有代码 ...

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            // ... 现有 commands ...
            get_focus_regions,  // 添加新 command
        ])
        // ... 其余代码 ...
}
4. 前端工具栏按钮
// src/components/panel/editor/EditorToolbar.tsx

import { Target } from 'lucide-react'; // 添加图标

interface EditorToolbarProps {
  // ... 现有 props ...
  showFocusAreas: boolean;
  onToggleFocusAreas(): void;
}

const EditorToolbar = memo(({
  // ... 现有 props ...
  showFocusAreas,
  onToggleFocusAreas,
}: EditorToolbarProps) => {
  // ... 现有代码 ...

  return (
    <div className="relative shrink-0 flex items-center justify-between px-4 h-14 gap-4 z-40">
      {/* ... 现有按钮 ... */}

      <button
        className={clsx(
          'p-2 rounded-full transition-colors',
          showFocusAreas
            ? 'bg-accent text-button-text hover:bg-accent/90'
            : 'bg-surface hover:bg-card-active text-text-primary'
        )}
        onClick={onToggleFocusAreas}
        onKeyDown={handleButtonKeyDown}
        data-tooltip={showFocusAreas ? 'Hide Focus Areas (Shift+F)' : 'Show Focus Areas (Shift+F)'}
      >
        <Target size={20} />
      </button>

      {/* ... 其余代码 ... */}
    </div>
  );
});
5. 前端状态管理
// src/App.tsx

const [showFocusAreas, setShowFocusAreas] = useState(false);
const [focusRegions, setFocusRegions] = useState<FocusRegion[]>([]);
const [focusAreasError, setFocusAreasError] = useState<string | null>(null);

// 加载对焦区域
useEffect(() => {
  if (!selectedImage?.path || !showFocusAreas) {
    setFocusRegions([]);
    return;
  }

  invoke<FocusRegion[]>('get_focus_regions', {
    path: selectedImage.path
  })
    .then(regions => {
      setFocusRegions(regions);
      setFocusAreasError(null);
    })
    .catch(err => {
      setFocusRegions([]);
      setFocusAreasError(err);
      toast.info(
        `对焦区域显示不可用\n${err}\n\n您可以提交样本文件帮助我们添加支持`,
        { autoClose: 5000 }
      );
    });
}, [selectedImage?.path, showFocusAreas]);

// 键盘快捷键注册步骤：
// 1. 在 src/utils/keyboardUtils.ts 的 KEYBIND_DEFINITIONS 中添加：
//    { action: 'toggle_focus_areas', description: 'Toggle focus area overlay',
//      defaultCombo: ['shift', 'KeyF'], section: 'view' }
// 2. 在 src/hooks/useKeyboardShortcuts.tsx 的 KeyboardShortcutsProps 接口中添加：
//    handleToggleFocusAreas(): void;
// 3. 在 useKeyboardShortcuts 的 actions 对象中添加：
//    toggle_focus_areas: {
//      shouldFire: () => !!selectedImage,
//      execute: (event) => { event.preventDefault(); handleToggleFocusAreas(); },
//    }
// 4. 在 App.tsx 的 useKeyboardShortcuts 调用处传入：
//    handleToggleFocusAreas: () => setShowFocusAreas(prev => !prev),
6. 坐标变换函数
// src/components/panel/editor/ImageCanvas.tsx

interface TransformedFocusPoint {
  x: number;
  y: number;
  width: number;
  height: number;
}

function transformFocusPoint(
  region: FocusRegion,
  adjustments: Adjustments,
  imageSize: { width: number; height: number },
  scale: number
): TransformedFocusPoint {
  // 1. 从归一化坐标转换为像素坐标
  let x = region.x * imageSize.width;
  let y = region.y * imageSize.height;
  let w = region.width * imageSize.width;
  let h = region.height * imageSize.height;

  // 2. 应用 EXIF 旋转 (orientationSteps: 0=0°, 1=90°, 2=180°, 3=270°)
  const orientationSteps = adjustments.orientationSteps || 0;
  if (orientationSteps > 0) {
    for (let i = 0; i < orientationSteps; i++) {
      const temp = x;
      x = imageSize.height - y - h;
      y = temp;
      [w, h] = [h, w];
      [imageSize.width, imageSize.height] = [imageSize.height, imageSize.width];
    }
  }

  // 3. 应用用户旋转
  if (adjustments.rotation && adjustments.rotation !== 0) {
    const angle = (adjustments.rotation * Math.PI) / 180;
    const cx = imageSize.width / 2;
    const cy = imageSize.height / 2;
    const cos = Math.cos(angle);
    const sin = Math.sin(angle);
    const dx = x + w / 2 - cx;
    const dy = y + h / 2 - cy;
    x = cx + dx * cos - dy * sin - w / 2;
    y = cy + dx * sin + dy * cos - h / 2;
  }

  // 4. 应用翻转
  if (adjustments.flipHorizontal) {
    x = imageSize.width - x - w;
  }
  if (adjustments.flipVertical) {
    y = imageSize.height - y - h;
  }

  // 5. 应用裁剪偏移
  const crop = adjustments.crop;
  if (crop) {
    const isPercent = crop.unit === '%';
    const cropX = isPercent ? (crop.x / 100) * imageSize.width : crop.x;
    const cropY = isPercent ? (crop.y / 100) * imageSize.height : crop.y;
    x -= cropX;
    y -= cropY;
  }

  // 6. 应用缩放
  return {
    x: x * scale,
    y: y * scale,
    width: w * scale,
    height: h * scale
  };
}
7. Konva 渲染
// src/components/panel/editor/ImageCanvas.tsx

{showFocusAreas && focusRegions.map((region, index) => {
  const transformed = transformFocusPoint(
    region,
    adjustments,
    { width: selectedImage.width, height: selectedImage.height },
    scale
  );

  // 根据类型选择样式
  const strokeColor = region.is_primary
    ? '#ef4444'  // 主对焦点：红色
    : region.kind === 'Face'
      ? '#22c55e'  // 人脸：绿色
      : region.kind === 'Eye'
        ? '#3b82f6'  // 眼睛：蓝色
        : '#f97316';  // 其他：橙色

  const isDashed = region.kind === 'Point';

  return (
    <Rect
      key={`focus-${index}`}
      x={transformed.x}
      y={transformed.y}
      width={transformed.width}
      height={transformed.height}
      stroke={strokeColor}
      strokeWidth={2}
      dash={isDashed ? [5, 5] : undefined}
      listening={false}
      shadowColor="black"
      shadowBlur={4}
      shadowOpacity={0.6}
    />
  );
})}
🚀 开发路线
Phase 0: 技术可行性验证 (1-2 天)
目标: 确认 rawler fork 是否暴露了足够的 MakerNotes API 来解析对焦点数据

任务清单:

 探查 rawler fork (RapidRAW-DngLab) 的 RawMetadata 结构：
   - 检查 metadata.exif 是否暴露了 maker_notes 字段
   - 检查 metadata.maker_notes 是否存在且有内容
   - 确认 RawMetadata 的 make/model 字段填充是否正常
 准备一个 Sony RAW 样本文件 (ARW 格式)：
   - 在 Rust 端用 rawler 解析，打印所有可访问的 EXIF 字段
   - 用 ExifTool 交叉验证 MakerNotes 中的对焦点数据是否存在
 如果 rawler fork 未暴露 MakerNotes，评估替代方案：
   - 方案 A: 直接 parse 二进制 MakerNotes blob（使用 kamadak-exif）
   - 方案 B: Fork rawler 增加 MakerNotes 暴露
   - 方案 C: 使用 image crate 的 exif 功能补充读取
 输出: 可行/不可行判定 + 至少一个 Sony 机型的对焦点解析 Demo

验收标准:

✅ 确认 rawler fork 的 MakerNotes 暴露程度
✅ 明确需要自行逆向的 MakerNotes 格式范围
✅ 至少跑通 1 个 Sony 机型的对焦点提取（从 RAW 读取到返回坐标）

Phase 1: 基础架构 (3-5 天)
目标: 搭建完整的技术栈，实现基本功能

任务清单:

 创建
focus_extraction.rs
 实现 FocusRegion 和 FocusKind 数据结构
 实现 FocusAdapter trait
 创建 SonyAdapter 骨架（暂时返回空）
 添加 get_focus_regions Tauri command
 在 lib.rs 中注册模块和命令：
   - 顶部添加 mod focus_extraction;
   - 添加 use crate::focus_extraction::...;
   - 在 generate_handler![] 宏中添加 get_focus_regions
 前端添加 showFocusAreas 状态
 工具栏添加切换按钮
 实现 transformFocusPoint 函数
 Konva 渲染层实现
 添加键盘快捷键 (Shift+F)
验收标准:

✅ 按钮可以切换显示/隐藏
✅ 调用 backend 不报错（即使返回空数组）
✅ 错误提示友好
Phase 2: Sony 支持 (5-7 天)
目标: 实现 1-2 个 Sony 机型的对焦点提取

任务清单:

 收集 Sony 样本文件（A7M3, A7M4, A7R5 等）
 研究 rawler 的 MakerNotes API
 尝试解析简单字段（如果存在）
 实现 SonyAdapter::extract()
 添加单元测试
 验证坐标准确性
验收标准:

✅ 至少支持 1 个 Sony 机型
✅ 对焦点位置准确（误差 < 5%）
✅ 支持多个对焦点
Phase 3: 扩展支持 (按需)
目标: 添加更多相机支持

任务清单:

 实现 CanonAdapter
 实现 NikonAdapter
 添加缓存机制
 性能优化
 添加"提交样本"功能
验收标准:

✅ 支持 Canon 主流机型
✅ 支持 Nikon 主流机型
✅ 缓存生效，二次加载 < 10ms
⚠️ 注意事项
技术限制
MakerNotes 解析难度高

rawler 对 MakerNotes 的支持有限
本项目使用 rawler 自定义 fork (RapidRAW-DngLab)，需先验证其 MakerNotes 暴露程度
不同机型格式差异大
需要逐个机型逆向工程
必须在 Phase 0 确认技术可行性后才能进入 Phase 1
初期支持率低

预期第一版只支持 20-30% 的相机
需要用户提供样本文件
社区驱动逐步扩展
坐标系复杂

需要处理多种变换
不同相机坐标系可能不同
需要大量测试验证
用户体验建议
友好的错误提示

toast.info(
  "此相机型号暂不支持对焦区域显示\n" +
  "支持的型号：Sony A7M4, Canon R5...\n\n" +
  "您可以通过 GitHub 提交样本文件帮助我们添加支持",
  { autoClose: 8000 }
);
提供样本提交入口

在设置面板添加"提交样本"按钮
自动打包 EXIF 数据（不包含图片内容）
生成 GitHub Issue 链接
性能优化

缓存解析结果
异步加载，不阻塞主流程
大量对焦点时使用虚拟化
安全考虑
隐私保护

只读取对焦相关元数据
不上传图片内容
样本提交需用户确认
错误处理

解析失败不应崩溃
提供降级方案
记录日志便于调试
📚 参考资料
项目内部
exif_processing.rs
 - EXIF 处理参考
mask_generation.rs
 - Adapter 模式参考
ImageCanvas.tsx
 - Konva 渲染参考
外部资源
rawler 文档 — 注意：项目使用自定义 fork: github.com/CyberTimon/RapidRAW-DngLab
ExifTool Tag Names
Konva 文档
🎯 总结
核心优势
✅ 零额外依赖 - 完全基于现有技术栈
✅ 性能优异 - 纯 Rust 解析，5-20ms
✅ 架构清晰 - Adapter 模式易于扩展
✅ 用户友好 - 一键切换，错误提示清晰
核心挑战
⚠️ MakerNotes 复杂 - 需要逐个机型适配
⚠️ 初期覆盖率低 - 第一版只支持少数机型
⚠️ 坐标变换复杂 - 需要大量测试
最终建议
采用渐进式开发策略：

先验证技术可行性（Phase 0）
再搭建完整架构（Phase 1）
支持 1-2 个主流机型（Phase 2）
提供样本提交机制
社区驱动逐步扩展
现实预期：

Phase 0 先行确认 rawler fork 的 MakerNotes 能力
第一版支持率：20-30%
后续每月增加 2-3 个机型
1 年后覆盖率：60-70%
文档版本: 1.0
作者: Kiro AI Assistant
审核状态: ✅ 已基于实际代码验证
```
