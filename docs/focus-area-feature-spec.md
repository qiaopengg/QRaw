# 对焦区域标注功能技术实现文档

> 版本: 2.0
> 最后更新: 2026-05-11
> 状态: 已按非侵入 feature 架构实现
> 约束: 必须遵守 `AGENTS.md` 和 `docs/feature-integration-guidelines.md`

## 功能目标

在不使用 AI 推测的前提下，从相机 RAW 元数据中提取实际对焦区域，并在编辑器画布中以叠加层方式展示。

## 非侵入架构

本功能是 QRaw 的二开 feature，不允许把业务实现散落到 RapidRAW 上游核心文件中。

### 前端位置

```text
src/features/focus-areas/
  constants.ts
  contracts.ts
  feature.tsx
  index.ts
  types.ts
  useFocusAreas.tsx
  FocusAreaToolbarButton.tsx
  FocusAreasToolbarEntry.tsx
  FocusAreasCanvasEntry.tsx
  FocusAreasOverlay.tsx
```

### Rust 位置

```text
src-tauri/src/features/
  mod.rs
  focus_areas/
    mod.rs
```

### 允许修改的上游挂载点

上游文件只允许存在通用挂载点：

- `src/App.tsx`: 调用 `useAppFeatures`，透传 `editorFeatureSlots` 和 `extraActions`
- `src/components/panel/Editor.tsx`: 接收并透传 `editorFeatureSlots`
- `src/components/panel/editor/EditorToolbar.tsx`: 渲染 `toolbarControls` 插槽
- `src/components/panel/editor/ImageCanvas.tsx`: 渲染 `imageCanvasOverlays` 插槽
- `src/hooks/useKeyboardShortcuts.tsx`: 合并通用 `extraActions`
- `src/utils/keyboardUtils.ts`: 从统一聚合入口读取 feature keybind 定义
- `src-tauri/src/lib.rs`: 只注册 `features::get_focus_regions`

这些文件不得包含对焦区域的状态、解析、渲染细节或厂商适配逻辑。

## 前端实现

### feature 注册

`src/features/focus-areas/feature.tsx` 返回统一注册对象：

```tsx
export function useFocusAreaFeature(context: AppFeatureKeyboardContext): AppFeatureRegistration {
  const focusAreas = useFocusAreas(context.selectedImage);

  return {
    editor: {
      imageCanvasOverlays: [(props) => <FocusAreasCanvasEntry {...props} focusAreas={focusAreas} />],
      toolbarControls: [(props) => <FocusAreasToolbarEntry {...props} focusAreas={focusAreas} />],
    },
    keyboardActions: createFocusAreaShortcutActions(context.selectedImage, focusAreas.toggleFocusAreas),
  };
}
```

`src/features/appFeatures.ts` 负责聚合所有 feature。

### 状态管理

`useFocusAreas.tsx` 独立管理：

- `showFocusAreas`
- `focusRegions`
- `focusAreasError`
- `toggleFocusAreas`
- `get_focus_regions` IPC 调用

### 工具栏

`FocusAreaToolbarButton.tsx` 渲染 Target 图标按钮，`FocusAreasToolbarEntry.tsx` 将按钮挂入 toolbar slot。

### 画布叠加层

`FocusAreasOverlay.tsx` 负责：

- 坐标变换
- Konva `Stage/Layer/Rect` 渲染
- 按 `kind` 和 `is_primary` 选择颜色
- 点对焦虚线样式
- 黑色阴影和 `listening={false}`

## 后端实现

### Command 入口

`src-tauri/src/features/mod.rs` 提供统一导出：

```rust
#[tauri::command]
pub fn get_focus_regions(
    params: focus_areas::GetFocusRegionsParams,
) -> Result<Vec<focus_areas::FocusRegion>, String> {
    focus_areas::get_focus_regions(params)
}
```

`src-tauri/src/lib.rs` 只注册：

```rust
features::get_focus_regions,
```

### 解析流程

`src-tauri/src/features/focus_areas/mod.rs` 内部流程：

1. `parse_virtual_path` 解析虚拟副本路径
2. 查询 `FocusCache`
3. 优先调用 ExifTool 解析厂商对焦标签
4. 回退到标准 EXIF `SubjectArea` / `SubjectLocation`
5. 回退到内置 MakerNote 解析
6. 统一返回 `Vec<FocusRegion>`

### 常见标签来源

- Sony: `FlexibleSpotPosition`、`FocalPlaneAFPointLocation*`、`FocusLocation`
- Canon: `AFImageWidth`、`AFImageHeight`、`AFAreaXPositions`、`AFAreaYPositions`、`AFPointsInFocus`
- Nikon: `AFImageWidth`、`AFImageHeight`、`AFAreaXPosition`、`AFAreaYPosition`、`AFPointsInFocus`
- Fujifilm: `FocusPixel`
- 标准 EXIF: `SubjectArea`、`SubjectLocation`

## 数据结构

```rust
pub struct FocusRegion {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub kind: FocusKind,
    pub is_primary: bool,
}
```

```ts
export type FocusKind = 'point' | 'area' | 'face' | 'eye';

export interface FocusRegion {
  x: number;
  y: number;
  width: number;
  height: number;
  kind: FocusKind;
  is_primary: boolean;
}
```

## 坐标变换

前端叠加层按当前图像显示管线处理：

1. 归一化坐标转像素坐标
2. `orientationSteps` 粗旋转
3. 水平翻转
4. 垂直翻转
5. 用户任意角度旋转
6. 裁剪偏移
7. 画布缩放

## 快捷键

快捷键定义必须先位于 feature 目录：

```ts
export const FOCUS_AREA_KEYBIND_DEFINITION = {
  action: 'toggle_focus_areas',
  description: 'Toggle focus area overlay',
  defaultCombo: ['shift', 'KeyF'],
  section: 'view',
};
```

再由 `src/features/keybindDefinitions.ts` 统一聚合。公共快捷键层不得直接导入 `src/features/focus-areas/constants.ts` 以外的实现细节。

## 开发约束

后续修改本功能时：

- 前端主体只改 `src/features/focus-areas/`
- Rust 主体只改 `src-tauri/src/features/focus_areas/`
- 新增 UI 入口通过 `EditorFeatureSlots`
- 新增快捷键通过 `FEATURE_KEYBIND_DEFINITIONS` 和 `extraActions`
- 新增 command 通过 `src-tauri/src/features/mod.rs` 包装后注册
- 不得恢复旧的 `src-tauri/src/focus_extraction.rs`

## 验收扫描

```bash
rg -n "focusAreas|showFocusAreas|focusRegions|FocusRegion|GetFocusRegions|toggle_focus_areas|get_focus_regions|FOCUS_AREAS|FocusAreas|FocusArea|对焦区域" \
  src src-tauri \
  -g '!src/features/**' \
  -g '!src-tauri/src/features/**' \
  -g '!src-tauri/lensfun_db/**'
```

合格结果应只出现统一入口或通用插槽，不应出现对焦业务实现散落。

## 验证命令

```bash
npm run build
cargo check
```
