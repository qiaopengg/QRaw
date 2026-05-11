# 对焦区域标注功能设计文档

## 目标

对焦区域标注是 QRaw 在 RapidRAW 上的二次开发功能。它通过读取 RAW 文件中的相机对焦元数据，在编辑器画布上显示相机拍摄时的实际对焦点或对焦区域。

本功能必须保持非侵入式接入：真实实现只允许位于独立 feature 目录，上游代码仅保留统一注册、插槽和命令挂载点。

## 架构原则

- 前端主体集中在 `src/features/focus-areas/`
- Rust 主体集中在 `src-tauri/src/features/focus_areas/`
- 上游 React 组件只提供通用 feature slot，不包含对焦业务逻辑
- `src-tauri/src/lib.rs` 只注册 `features::get_focus_regions`
- 新增能力必须扩展 feature 目录或统一 feature 架构，不得回写到上游核心组件
- 本文档必须与 `docs/feature-integration-guidelines.md` 保持一致

## 当前目录

```text
src/features/
  appFeatures.ts
  contracts.ts
  keybindDefinitions.ts
  focus-areas/
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

src-tauri/src/features/
  mod.rs
  focus_areas/
    mod.rs
```

## 前端设计

### 统一注册

`src/features/appFeatures.ts` 聚合所有二开 feature。对焦区域功能通过 `useFocusAreaFeature()` 注册：

- `editor.toolbarControls`: 向编辑器工具栏注入对焦按钮
- `editor.imageCanvasOverlays`: 向画布注入对焦叠加层
- `keyboardActions`: 注入 `toggle_focus_areas` 快捷键动作

`App.tsx` 只调用统一入口：

```tsx
const appFeatures = useAppFeatures({ selectedImage });
```

并将结果透传给公共层：

```tsx
useKeyboardShortcuts({
  // ...
  extraActions: appFeatures.keyboardActions,
});

<Editor editorFeatureSlots={appFeatures.editor ?? {}} />
```

### 状态与数据加载

状态全部由 `src/features/focus-areas/useFocusAreas.tsx` 管理：

- `showFocusAreas`
- `focusRegions`
- `focusAreasError`
- `toggleFocusAreas()`
- `invoke('get_focus_regions')`

`App.tsx` 不保存对焦区域状态，不直接调用对焦区域 command。

### 工具栏按钮

按钮由 `FocusAreaToolbarButton.tsx` 实现，并通过 `FocusAreasToolbarEntry.tsx` 挂载到 `EditorToolbar` 的 `toolbarControls` 插槽。

`EditorToolbar.tsx` 只保留通用插槽渲染：

```tsx
{editorFeatureSlots.toolbarControls?.map((ToolbarControl, index) => (
  <ToolbarControl key={`editor-toolbar-feature-${index}`} onKeyDown={handleButtonKeyDown} />
))}
```

### 画布叠加层

对焦叠加层由 `FocusAreasOverlay.tsx` 实现，并通过 `FocusAreasCanvasEntry.tsx` 挂载到 `ImageCanvas` 的 `imageCanvasOverlays` 插槽。

`ImageCanvas.tsx` 只负责提供通用上下文：

```tsx
{editorFeatureSlots.imageCanvasOverlays?.map((ImageCanvasOverlay, index) => (
  <ImageCanvasOverlay
    key={`image-canvas-feature-${index}`}
    adjustments={adjustments}
    effectiveCursor={effectiveCursor}
    imageRenderSize={imageRenderSize}
    imageSize={{ width: selectedImage.width, height: selectedImage.height }}
    isShowingOriginal={isShowingOriginal}
  />
))}
```

### 坐标变换顺序

对焦区域坐标在 `FocusAreasOverlay.tsx` 内转换，顺序必须匹配当前图像显示管线：

1. 归一化坐标转换为像素坐标
2. 应用 `orientationSteps` 粗旋转
3. 应用水平翻转
4. 应用垂直翻转
5. 应用用户任意角度旋转
6. 应用裁剪偏移
7. 应用画布缩放

## 后端设计

### Command 暴露

`src-tauri/src/features/mod.rs` 暴露统一 command：

```rust
#[tauri::command]
pub fn get_focus_regions(
    params: focus_areas::GetFocusRegionsParams,
) -> Result<Vec<focus_areas::FocusRegion>, String> {
    focus_areas::get_focus_regions(params)
}
```

`src-tauri/src/lib.rs` 只允许注册该入口：

```rust
features::get_focus_regions,
```

不得在 `lib.rs` 中实现对焦解析、缓存、ExifTool 调用、厂商适配或坐标转换。

### 解析策略

`src-tauri/src/features/focus_areas/mod.rs` 负责全部业务实现：

1. 解析虚拟副本路径
2. 命中缓存则直接返回
3. 优先调用 ExifTool 读取常见厂商对焦标签
4. 回退到标准 EXIF `SubjectArea` / `SubjectLocation`
5. 再回退到内置 MakerNote 解析
6. 返回统一的 `FocusRegion` 列表

### 支持范围

当前策略面向常见相机 RAW 格式：

- Sony: `FlexibleSpotPosition`、`FocalPlaneAFPoint*`、`FocusLocation`、部分 MakerNote fallback
- Canon: `AFImageWidth/Height`、`AFArea*`、`AFPointsInFocus`
- Nikon: `AFImageWidth/Height`、`AFArea*`、`AFPointsInFocus`
- Fujifilm 等: 优先尝试 `FocusPixel`
- 标准 EXIF: `SubjectArea`、`SubjectLocation`

非 RAW 文件不作为承诺支持范围；只有当元数据工具可提供有效对焦字段时才可能返回结果。

## 数据契约

后端返回的 `FocusRegion` 与前端 `src/features/focus-areas/types.ts` 保持一致：

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

坐标统一为 0.0 到 1.0 的归一化值，表示相对于当前解析基准图像的左上角和宽高。

## 非侵入检查清单

每次修改对焦区域功能后必须检查：

```bash
rg -n "focusAreas|showFocusAreas|focusRegions|FocusRegion|GetFocusRegions|toggle_focus_areas|get_focus_regions|FOCUS_AREAS|FocusAreas|FocusArea|对焦区域" \
  src src-tauri \
  -g '!src/features/**' \
  -g '!src-tauri/src/features/**' \
  -g '!src-tauri/lensfun_db/**'
```

预期结果只能包含通用入口或注册点，例如：

- `src-tauri/src/lib.rs` 中的 `features::get_focus_regions`
- `App.tsx` 中的 `useAppFeatures`
- `Editor.tsx`、`EditorToolbar.tsx`、`ImageCanvas.tsx` 中的通用 `editorFeatureSlots`
- `useKeyboardShortcuts.tsx` 中的通用 `extraActions`

如果在 `src/components/**`、`src/hooks/**`、`src/utils/**` 中出现具体对焦业务实现，应迁回 `src/features/focus-areas/`。

## 禁止事项

- 禁止新建或恢复 `src-tauri/src/focus_extraction.rs`
- 禁止在 `App.tsx` 中直接维护 `showFocusAreas` 或 `focusRegions`
- 禁止在 `ImageCanvas.tsx` 中直接实现对焦坐标变换或对焦渲染
- 禁止在 `EditorToolbar.tsx` 中直接导入 `Target` 并实现对焦按钮
- 禁止在 `useKeyboardShortcuts.tsx` 中写死 `toggle_focus_areas` 业务动作
- 禁止在 `src-tauri/src/lib.rs` 中直接实现 `get_focus_regions`

## 验证

推荐修改后运行：

```bash
npm run build
cargo check
```

并执行上文的非侵入扫描，确认对焦功能实现仍集中在 feature 目录。
