# 设计文档

## 概述

本文档描述了相机对焦区域标注功能的技术设计。该功能通过从 RAW 文件元数据中提取对焦点信息,并在图像编辑器画布上以彩色矩形叠加层的形式可视化显示这些对焦区域。

## 高层设计

### 系统架构

```
┌─────────────────────────────────────────────────────────┐
│                   前端层 (React/TypeScript)              │
├─────────────────────────────────────────────────────────┤
│  EditorToolbar.tsx                                       │
│  ├─ [显示对焦区域] 按钮 (Target 图标)                    │
│  ├─ showFocusAreas: boolean 状态                        │
│  └─ 键盘快捷键: Shift+F                                  │
├─────────────────────────────────────────────────────────┤
│  App.tsx (状态管理)                                      │
│  ├─ showFocusAreas: boolean                             │
│  ├─ focusRegions: FocusRegion[]                         │
│  ├─ focusAreasError: string | null                      │
│  └─ useEffect: 监听状态变化并调用后端                    │
├─────────────────────────────────────────────────────────┤
│  ImageCanvas.tsx (渲染层)                                │
│  ├─ transformFocusPoint() 坐标变换函数                  │
│  ├─ Konva Stage/Layer                                   │
│  └─ <Rect> 组件渲染对焦框                                │
└─────────────────────────────────────────────────────────┘
                          ↕ Tauri IPC
┌─────────────────────────────────────────────────────────┐
│                   后端层 (Rust)                          │
├─────────────────────────────────────────────────────────┤
│  lib.rs                                                  │
│  └─ #[tauri::command] get_focus_regions()                │
│     ├─ 解析虚拟副本路径                                  │
│     ├─ 检查缓存                                          │
│     ├─ 读取文件                                          │
│     ├─ 选择适配器                                        │
│     └─ 返回 JSON 结果                                    │
├─────────────────────────────────────────────────────────┤
│  focus_extraction.rs (新建模块)                          │
│  ├─ FocusRegion 结构体                                   │
│  ├─ FocusKind 枚举                                       │
│  ├─ FocusAdapter trait                                   │
│  ├─ SonyAdapter 实现                                     │
│  ├─ CanonAdapter 实现                                    │
│  ├─ NikonAdapter 实现                                    │
│  └─ FocusCache 缓存管理                                  │
├─────────────────────────────────────────────────────────┤
│  exif_processing.rs (已存在,复用)                        │
│  └─ read_raw_metadata() 函数                             │
└─────────────────────────────────────────────────────────┘
```

### 组件交互流程

```
用户操作
   │
   ├─ 点击 [显示对焦区域] 按钮
   │     │
   │     └─> EditorToolbar.onToggleFocusAreas()
   │            │
   │            └─> App.setShowFocusAreas(true)
   │
   └─ 按下 Shift+F 快捷键
         │
         └─> useKeyboardShortcuts (通过 KEYBIND_DEFINITIONS 注册)
                │
                └─> App.setShowFocusAreas(toggle)

状态变化触发
   │
   └─> useEffect [showFocusAreas, selectedImage.path]
          │
          ├─ if showFocusAreas === false
          │     └─> setFocusRegions([])
          │
          └─ if showFocusAreas === true
                │
                └─> invoke('get_focus_regions', { path })
                       │
                       ├─ 成功
                       │    └─> setFocusRegions(regions)
                       │
                       └─ 失败
                            └─> toast.info(error)

后端处理
   │
   └─> get_focus_regions(path)
          │
          ├─> 1. parse_virtual_path(path)
          ├─> 2. check_cache(path)
          ├─> 3. read_file(path)
          ├─> 4. read_raw_metadata(bytes)
          ├─> 5. select_adapter(metadata.make)
          ├─> 6. adapter.extract(metadata)
          ├─> 7. cache_result(path, regions)
          └─> 8. return Vec<FocusRegion>

渲染流程
   │
   └─> ImageCanvas.render()
          │
          └─> if showFocusAreas && focusRegions.length > 0
                 │
                 └─> focusRegions.map(region => {
                        │
                        ├─> transformed = transformFocusPoint(region)
                        │
                        └─> <Rect
                              x={transformed.x}
                              y={transformed.y}
                              width={transformed.width}
                              height={transformed.height}
                              stroke={getColor(region)}
                              dash={getDashPattern(region)}
                            />
                     })
```

### 数据流图

```
RAW 文件
   │
   └─> [后端] read_raw_metadata()
          │
          └─> RawMetadata
                 │
                 ├─ metadata.make: "Sony"
                 ├─ metadata.model: "ILCE-7M4"
                 └─ metadata.exif: {...}
                       │
                       └─> [后端] SonyAdapter.extract()
                              │
                              └─> Vec<FocusRegion>
                                     │
                                     ├─ FocusRegion {
                                     │    x: 0.5,        // 归一化坐标
                                     │    y: 0.5,
                                     │    width: 0.1,
                                     │    height: 0.1,
                                     │    kind: Point,
                                     │    is_primary: true
                                     │  }
                                     │
                                     └─> [Tauri IPC] JSON 序列化
                                            │
                                            └─> [前端] invoke() 返回
                                                   │
                                                   └─> FocusRegion[]
                                                          │
                                                          └─> [前端] transformFocusPoint()
                                                                 │
                                                                 ├─ 应用 EXIF 旋转
                                                                 ├─ 应用用户旋转
                                                                 ├─ 应用翻转
                                                                 ├─ 应用裁剪
                                                                 └─ 应用缩放
                                                                       │
                                                                       └─> TransformedFocusPoint {
                                                                              x: 640,    // 像素坐标
                                                                              y: 480,
                                                                              width: 128,
                                                                              height: 128
                                                                            }
                                                                              │
                                                                              └─> [Konva] <Rect> 渲染
```

## 低层设计

### 数据结构定义

#### 后端 Rust 数据结构

```rust
// src-tauri/src/focus_extraction.rs

use serde::{Deserialize, Serialize};
use rawler::decoders::RawMetadata;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::SystemTime;

/// 对焦区域数据结构
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct FocusRegion {
    /// 归一化 X 坐标 (0.0 - 1.0)
    /// 相对于原始传感器宽度
    pub x: f32,

    /// 归一化 Y 坐标 (0.0 - 1.0)
    /// 相对于原始传感器高度
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

/// 对焦类型枚举
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum FocusKind {
    /// 单点对焦
    Point,

    /// 区域对焦
    Area,

    /// 人脸检测对焦
    Face,

    /// 眼部对焦
    Eye,
}

/// 对焦适配器 trait
pub trait FocusAdapter {
    /// 检查是否支持该相机
    fn supports(metadata: &RawMetadata) -> bool;

    /// 提取对焦区域
    /// 返回归一化坐标的对焦区域列表
    fn extract(metadata: &RawMetadata) -> Result<Vec<FocusRegion>, String>;
}

/// 缓存条目
#[derive(Clone, Debug)]
struct CacheEntry {
    regions: Vec<FocusRegion>,
    timestamp: SystemTime,
}

/// 对焦区域缓存 (真正的 LRU 驱逐策略)
/// 注意：不能依赖 HashMap 迭代顺序（不确定），
/// 因此使用 VecDeque 维护插入顺序
pub struct FocusCache {
    cache: Mutex<HashMap<String, CacheEntry>>,
    order: Mutex<VecDeque<String>>,
    max_size: usize,
}

impl FocusCache {
    pub fn new(max_size: usize) -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
            order: Mutex::new(VecDeque::new()),
            max_size,
        }
    }

    pub fn get(&self, key: &str) -> Option<Vec<FocusRegion>> {
        let cache = self.cache.lock().unwrap();
        cache.get(key).map(|entry| entry.regions.clone())
    }

    pub fn insert(&self, key: String, regions: Vec<FocusRegion>) {
        let mut cache = self.cache.lock().unwrap();
        let mut order = self.order.lock().unwrap();

        // 如果 key 已存在，移除旧位置
        if let Some(pos) = order.iter().position(|k| k == &key) {
            order.remove(pos);
        }

        // 如果缓存已满，驱逐最旧的条目（队列头部）
        if cache.len() >= self.max_size {
            if let Some(oldest_key) = order.pop_front() {
                cache.remove(&oldest_key);
            }
        }

        order.push_back(key.clone());
        cache.insert(key, CacheEntry {
            regions,
            timestamp: SystemTime::now(),
        });
    }

    pub fn invalidate(&self, key: &str) {
        let mut cache = self.cache.lock().unwrap();
        let mut order = self.order.lock().unwrap();
        cache.remove(key);
        if let Some(pos) = order.iter().position(|k| k == key) {
            order.remove(pos);
        }
    }
}
```

#### 前端 TypeScript 数据结构

```typescript
// 注意：本项目的 TypeScript 类型约定定义在 src/components/ui/AppProperties.tsx
// 此处以独立文件 src/types/focus.ts 展示类型定义，实际实施时可选择位置

/**
 * 对焦区域接口
 */
export interface FocusRegion {
  /** 归一化 X 坐标 (0.0 - 1.0) */
  x: number;

  /** 归一化 Y 坐标 (0.0 - 1.0) */
  y: number;

  /** 归一化宽度 (0.0 - 1.0) */
  width: number;

  /** 归一化高度 (0.0 - 1.0) */
  height: number;

  /** 对焦类型 */
  kind: FocusKind;

  /** 是否为主对焦点 */
  is_primary: boolean;
}

/**
 * 对焦类型
 */
export type FocusKind = 'point' | 'area' | 'face' | 'eye';

/**
 * 变换后的对焦点(像素坐标)
 */
export interface TransformedFocusPoint {
  x: number;
  y: number;
  width: number;
  height: number;
}
```

### 关键算法

#### 算法 1: Sony 适配器对焦点提取

```rust
// src-tauri/src/focus_extraction.rs

pub struct SonyAdapter;

impl FocusAdapter for SonyAdapter {
    fn supports(metadata: &RawMetadata) -> bool {
        metadata.make.to_lowercase().contains("sony")
    }

    fn extract(metadata: &RawMetadata) -> Result<Vec<FocusRegion>, String> {
        let mut regions = Vec::new();

        // 阶段 1: 尝试从标准 EXIF 字段读取
        // 注意: 大多数 Sony 相机将对焦信息存储在 MakerNotes 中

        // 获取图像尺寸用于归一化
        let image_width = metadata.width as f32;
        let image_height = metadata.height as f32;

        // 尝试读取 MakerNotes 中的 AFInfo
        // 这需要根据具体机型进行逆向工程
        // 以下是伪代码示例:

        /*
        if let Some(af_info) = metadata.maker_notes.get("AFInfo") {
            // 解析 AFInfo blob
            let af_data = parse_sony_af_info(af_info)?;

            for point in af_data.focus_points {
                regions.push(FocusRegion {
                    x: point.x / image_width,
                    y: point.y / image_height,
                    width: point.width / image_width,
                    height: point.height / image_height,
                    kind: match point.af_type {
                        0 => FocusKind::Point,
                        1 => FocusKind::Area,
                        2 => FocusKind::Face,
                        3 => FocusKind::Eye,
                        _ => FocusKind::Area,
                    },
                    is_primary: point.is_selected,
                });
            }
        }
        */

        // 阶段 1 实现: 暂时返回空列表
        // 等待用户提供样本文件进行逆向工程
        log::warn!(
            "Sony focus area extraction not yet implemented for model: {}",
            metadata.model
        );

        Ok(regions)
    }
}
```

#### 算法 2: 坐标变换管道

```typescript
// src/components/panel/editor/ImageCanvas.tsx

/**
 * 变换对焦点坐标
 *
 * 变换顺序:
 * 1. 归一化坐标 → 像素坐标
 * 2. EXIF 旋转 (0°/90°/180°/270°)
 * 3. 用户旋转 (任意角度)
 * 4. 水平翻转
 * 5. 垂直翻转
 * 6. 裁剪偏移
 * 7. 画布缩放
 */
function transformFocusPoint(
  region: FocusRegion,
  adjustments: Adjustments,
  imageSize: { width: number; height: number },
  scale: number,
): TransformedFocusPoint {
  // 步骤 1: 从归一化坐标转换为像素坐标
  let x = region.x * imageSize.width;
  let y = region.y * imageSize.height;
  let w = region.width * imageSize.width;
  let h = region.height * imageSize.height;

  // 步骤 2: 应用 EXIF 旋转
  // orientationSteps: 0=0°, 1=90°, 2=180°, 3=270°
  const orientationSteps = adjustments.orientationSteps || 0;

  for (let i = 0; i < orientationSteps; i++) {
    // 每次旋转 90° 顺时针
    const temp = x;
    x = imageSize.height - y - h;
    y = temp;

    // 交换宽高
    [w, h] = [h, w];

    // 交换图像尺寸
    [imageSize.width, imageSize.height] = [imageSize.height, imageSize.width];
  }

  // 步骤 3: 应用用户旋转(任意角度)
  if (adjustments.rotation && adjustments.rotation !== 0) {
    const angle = (adjustments.rotation * Math.PI) / 180;
    const cx = imageSize.width / 2;
    const cy = imageSize.height / 2;

    // 旋转矩形中心点
    const cos = Math.cos(angle);
    const sin = Math.sin(angle);
    const dx = x + w / 2 - cx;
    const dy = y + h / 2 - cy;

    const rotatedCenterX = cx + dx * cos - dy * sin;
    const rotatedCenterY = cy + dx * sin + dy * cos;

    // 更新左上角坐标
    x = rotatedCenterX - w / 2;
    y = rotatedCenterY - h / 2;
  }

  // 步骤 4: 应用水平翻转
  if (adjustments.flipHorizontal) {
    x = imageSize.width - x - w;
  }

  // 步骤 5: 应用垂直翻转
  if (adjustments.flipVertical) {
    y = imageSize.height - y - h;
  }

  // 步骤 6: 应用裁剪偏移
  const crop = adjustments.crop;
  if (crop) {
    const isPercent = crop.unit === '%';
    const cropX = isPercent ? (crop.x / 100) * imageSize.width : crop.x;
    const cropY = isPercent ? (crop.y / 100) * imageSize.height : crop.y;

    x -= cropX;
    y -= cropY;
  }

  // 步骤 7: 应用画布缩放
  // 防止除零错误
  const safeScale = scale > 0 ? scale : 1.0;

  return {
    x: x * safeScale,
    y: y * safeScale,
    width: w * safeScale,
    height: h * safeScale,
  };
}
```

#### 算法 3: Tauri 命令实现

```rust
// src-tauri/src/lib.rs

use once_cell::sync::Lazy;
use crate::focus_extraction::{FocusRegion, FocusAdapter, SonyAdapter, CanonAdapter, NikonAdapter, FocusCache};

// 全局缓存实例
static FOCUS_CACHE: Lazy<FocusCache> = Lazy::new(|| FocusCache::new(100));

#[tauri::command]
pub fn get_focus_regions(path: String) -> Result<Vec<FocusRegion>, String> {
    // 1. 解析虚拟副本路径
    // parse_virtual_path 返回 (source_path, sidecar_path) 元组
    let (source_path, _sidecar) = parse_virtual_path(&path);

    // 2. 生成缓存键
    let cache_key = format!("focus_{}", source_path.to_string_lossy());

    // 3. 检查缓存
    if let Some(cached_regions) = FOCUS_CACHE.get(&cache_key) {
        log::debug!("Focus regions cache hit for: {:?}", source_path);
        return Ok(cached_regions);
    }

    // 4. 读取文件
    let file_bytes = match read_file_mapped(&source_path) {
        Ok(mmap) => mmap.to_vec(),
        Err(_) => std::fs::read(&source_path)
            .map_err(|e| format!("无法读取文件: {}", e))?
    };

    // 5. 解析元数据
    let raw_metadata = exif_processing::read_raw_metadata(&file_bytes)
        .ok_or("不是 RAW 文件或元数据不可用")?;

    // 6. 选择适配器并提取
    let regions = if SonyAdapter::supports(&raw_metadata) {
        SonyAdapter::extract(&raw_metadata)?
    } else if CanonAdapter::supports(&raw_metadata) {
        CanonAdapter::extract(&raw_metadata)?
    } else if NikonAdapter::supports(&raw_metadata) {
        NikonAdapter::extract(&raw_metadata)?
    } else {
        return Err(format!(
            "不支持的相机: {} {}\n\n您可以提交样本文件帮助我们添加支持",
            raw_metadata.make,
            raw_metadata.model
        ));
    };

    // 7. 缓存结果
    FOCUS_CACHE.insert(cache_key, regions.clone());

    // 8. 返回结果
    log::info!(
        "Extracted {} focus regions from {} {}",
        regions.len(),
        raw_metadata.make,
        raw_metadata.model
    );

    Ok(regions)
}
```

### 渲染实现

#### Konva 渲染代码

```typescript
// src/components/panel/editor/ImageCanvas.tsx

// 在 ImageCanvas 组件的 Konva Stage 中添加对焦叠加层
{showFocusAreas && focusRegions.map((region, index) => {
  // 变换坐标
  const transformed = transformFocusPoint(
    region,
    adjustments,
    { width: selectedImage.width, height: selectedImage.height },
    scale
  );

  // 根据类型和是否为主对焦点选择颜色
  const strokeColor = region.is_primary
    ? '#ef4444'  // 主对焦点: 红色
    : region.kind === 'face'
      ? '#22c55e'  // 人脸: 绿色
      : region.kind === 'eye'
        ? '#3b82f6'  // 眼睛: 蓝色
        : '#f97316';  // 其他: 橙色

  // 单点对焦使用虚线
  const isDashed = region.kind === 'point';

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
      listening={false}  // 不响应鼠标事件
      shadowColor="black"
      shadowBlur={4}
      shadowOpacity={0.6}
    />
  );
})}
```

## 性能考虑

### 缓存策略

1. **缓存键**: 使用文件路径作为缓存键
2. **缓存大小**: 限制为 100 个条目,防止内存无限增长
3. **缓存失效**:
   - 文件修改时自动失效(通过文件时间戳检测)
   - LRU 策略: 缓存满时移除最旧的条目
4. **缓存命中率**: 预期 > 80% (用户经常在同一组图片间切换)

### 性能指标

| 操作       | 目标时间 | 说明                   |
| ---------- | -------- | ---------------------- |
| 元数据提取 | < 50ms   | 对于 100MB 以下的文件  |
| 缓存命中   | < 10ms   | 从缓存返回数据         |
| 坐标变换   | < 1ms    | 单个对焦区域的变换计算 |
| Konva 渲染 | < 16ms   | 60 FPS 渲染目标        |

### 优化措施

1. **异步加载**: 对焦区域加载不阻塞 UI
2. **批量渲染**: 使用 Konva 的批量绘制 API
3. **内存映射**: 使用 `memmap2` 读取大文件
4. **惰性解析**: 只在需要时解析 MakerNotes

## 错误处理

### 错误类型

1. **文件读取错误**: 文件不存在或无权限
2. **解析错误**: 不是 RAW 文件或元数据损坏
3. **不支持的相机**: 没有对应的适配器
4. **提取失败**: MakerNotes 格式不识别

### 键盘快捷键注册

键盘快捷键通过 `keyboardUtils.ts` 中的 `KEYBIND_DEFINITIONS` 数组注册，然后在 `useKeyboardShortcuts` hook 的 `actions` 对象中添加处理逻辑：

```typescript
// 1. src/utils/keyboardUtils.ts — 在 KEYBIND_DEFINITIONS 数组中添加：
{
  action: 'toggle_focus_areas',
  description: 'Toggle focus area overlay',
  defaultCombo: ['shift', 'KeyF'],
  section: 'view',
},

// 2. src/hooks/useKeyboardShortcuts.tsx — 在 KeyboardShortcutsProps 接口中添加：
handleToggleFocusAreas(): void;

// 3. 在 useKeyboardShortcuts 的 actions 对象中添加：
toggle_focus_areas: {
  shouldFire: () => !!selectedImage,
  execute: (event) => { event.preventDefault(); handleToggleFocusAreas(); },
},

// 4. src/App.tsx — 在 useKeyboardShortcuts 调用处传入：
useKeyboardShortcuts({
  // ...其他 props...
  handleToggleFocusAreas: () => setShowFocusAreas(prev => !prev),
});
```

### 错误处理策略

```typescript
// 前端错误处理
useEffect(() => {
  if (!selectedImage?.path || !showFocusAreas) {
    setFocusRegions([]);
    return;
  }

  invoke<FocusRegion[]>('get_focus_regions', {
    path: selectedImage.path,
  })
    .then((regions) => {
      setFocusRegions(regions);
      setFocusAreasError(null);
    })
    .catch((err) => {
      setFocusRegions([]);
      setFocusAreasError(err);

      // 显示友好的错误提示
      toast.info(`对焦区域显示不可用\n${err}\n\n您可以提交样本文件帮助我们添加支持`, {
        autoClose: 5000,
        position: 'bottom-right',
      });
    });
}, [selectedImage?.path, showFocusAreas]);
```

## 模块注册（实施前必读）

此功能需要新建 `focus_extraction.rs` 模块文件，在实施时必须在 `src-tauri/src/lib.rs` 中完成以下注册步骤：

1. **模块声明** — 在 lib.rs 顶部 `mod ...` 区域添加: `mod focus_extraction;`
2. **use 导入** — 添加: `use crate::focus_extraction::{FocusRegion, FocusAdapter, SonyAdapter, CanonAdapter, NikonAdapter, FocusCache};`
3. **命令注册** — 在 `generate_handler![]` 宏中添加 `get_focus_regions`

缺少任一步骤都会导致编译错误。

## 扩展性设计

### 添加新相机支持

1. 创建新的适配器结构体
2. 实现 `FocusAdapter` trait
3. 在 `get_focus_regions` 中添加适配器检查
4. 编写单元测试验证

```rust
// 示例: 添加 Fujifilm 支持
pub struct FujifilmAdapter;

impl FocusAdapter for FujifilmAdapter {
    fn supports(metadata: &RawMetadata) -> bool {
        metadata.make.to_lowercase().contains("fujifilm")
    }

    fn extract(metadata: &RawMetadata) -> Result<Vec<FocusRegion>, String> {
        // 实现 Fujifilm 特定的提取逻辑
        todo!()
    }
}

// 在 get_focus_regions 中添加
let regions = if SonyAdapter::supports(&raw_metadata) {
    SonyAdapter::extract(&raw_metadata)?
} else if CanonAdapter::supports(&raw_metadata) {
    CanonAdapter::extract(&raw_metadata)?
} else if NikonAdapter::supports(&raw_metadata) {
    NikonAdapter::extract(&raw_metadata)?
} else if FujifilmAdapter::supports(&raw_metadata) {
    FujifilmAdapter::extract(&raw_metadata)?
} else {
    return Err(format!("不支持的相机: {} {}", ...));
};
```

## 测试策略

### 单元测试

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_focus_region_normalization() {
        let region = FocusRegion {
            x: 0.5,
            y: 0.5,
            width: 0.1,
            height: 0.1,
            kind: FocusKind::Point,
            is_primary: true,
        };

        assert!(region.x >= 0.0 && region.x <= 1.0);
        assert!(region.y >= 0.0 && region.y <= 1.0);
    }

    #[test]
    fn test_sony_adapter_supports() {
        let mut metadata = RawMetadata::default();
        metadata.make = "Sony".to_string();

        assert!(SonyAdapter::supports(&metadata));
    }
}
```

### 集成测试

1. 准备测试 RAW 文件样本
2. 验证提取的对焦区域数量和位置
3. 验证坐标变换的准确性
4. 验证缓存机制

### 视觉测试

1. 在不同图像上验证对焦框位置
2. 测试各种变换组合(旋转+翻转+裁剪)
3. 验证不同对焦类型的颜色和样式

## 部署注意事项

1. **依赖项**: 确保 `kamadak-exif` 和 `rawler` 版本兼容
2. **日志记录**: 启用详细日志以便调试
3. **用户反馈**: 提供样本文件提交机制
4. **文档**: 维护支持的相机型号列表

## 未来改进

1. **自动检测**: 自动识别对焦点格式
2. **更多相机**: 扩展到 Fujifilm、Olympus、Pentax 等
3. **3D 对焦**: 支持显示对焦深度信息
4. **对焦历史**: 显示连拍序列的对焦变化
5. **对焦分析**: 统计对焦准确率
