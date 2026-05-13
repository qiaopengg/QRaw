# 智能选图技术架构与选型

> 版本: 0.1
> 日期: 2026-05-12
> 状态: 架构设计
> 约束: 必须遵守 `AGENTS.md` 和 `docs/feature-integration-guidelines.md`
> 需求来源: `docs/smart-culling-feature-spec.md`

## 目标

智能选图需要在完全离线、本地推理的前提下，对当前文件夹或用户选择范围内的 RAW 照片完成初筛、打星、精选/待确认/淘汰建议、相似组折叠、人像表情判断、原因解释、复核确认、PDF 报告和一键撤销。

本功能必须作为独立二开 feature 落地，不允许把模型推理、评分、分组、报告、复核等业务散落到上游 `CyberTimon/RapidRAW` 代码中。

## 调研依据

- Tauri 2 官方文档支持从前端调用 Rust command，异步 command 适合重任务，事件可从 Rust 推送到前端；Channels 可用于流式数据。见 [Calling Rust from the Frontend](https://v2.tauri.app/develop/calling-rust/) 和 [Calling the Frontend from Rust](https://v2.tauri.app/develop/calling-frontend/)。
- Tauri 2 官方文档建议通过 `manage` 管理应用状态，必要时用 `Mutex` 做内部可变状态。见 [State Management](https://v2.tauri.app/develop/state-management/)。
- ONNX Runtime 官方文档说明 Execution Provider 可按硬件能力把子图分配到 CPU/GPU/NPU 等后端，并按 provider 优先级回退。见 [ONNX Runtime Execution Providers](https://onnxruntime.ai/docs/execution-providers/)。
- ONNX Runtime 官方文档确认 CoreML EP 可在 Apple CPU/GPU/Neural Engine 上运行，DirectML EP 可在 Windows 广泛 GPU 硬件上运行，CUDA EP 可使用 NVIDIA CUDA GPU。见 [CoreML EP](https://onnxruntime.ai/docs/execution-providers/CoreML-ExecutionProvider.html)、[DirectML EP](https://onnxruntime.ai/docs/execution-providers/DirectML-ExecutionProvider.html)、[CUDA EP](https://onnxruntime.ai/docs/execution-providers/CUDA-ExecutionProvider.html)。
- `ort` 是 Rust 的 ONNX Runtime 绑定，并提供 `Session`、Execution Provider 等能力；本项目已经依赖 `ort`。见 [ort docs.rs](https://docs.rs/ort/latest/ort/)。
- `image_hasher` 提供感知哈希算法和 Hamming distance，适合快速相似图预分组；本项目已经依赖 `image_hasher`。见 [image_hasher docs.rs](https://docs.rs/image_hasher/latest/image_hasher/)。
- CLIP 论文说明图文对比学习得到的图像表示可迁移到大量视觉任务。见 [Learning Transferable Visual Models From Natural Language Supervision](https://arxiv.org/abs/2103.00020)。
- OpenCLIP 提供开源 CLIP 实现和预训练模型接口；可作为离线图像 embedding 模型来源之一。见 [OpenCLIP](https://github.com/mlfoundations/open_clip)。
- LAION aesthetic predictor 是基于 CLIP embedding 的线性美学质量估计器，适合作为轻量美学 head 的参考实现。见 [LAION-AI/aesthetic-predictor](https://github.com/LAION-AI/aesthetic-predictor)。
- NIMA 论文提出无参考图像质量评估，可预测人类评分分布，适合作为美学/质量评分备选路线。见 [NIMA: Neural Image Assessment](https://arxiv.org/abs/1709.05424)。
- InsightFace model zoo 提供 SCRFD/RetinaFace/landmark 等模型信息，但其模型标注为仅限非商业研究用途，不能默认随应用分发。见 [InsightFace Model Zoo](https://github.com/deepinsight/insightface/tree/master/model_zoo)。
- MediaPipe Face Detector 官方文档说明 BlazeFace 系列是轻量人脸检测模型，可输出 bounding boxes 和 6 个关键点，并面向移动/本地推理优化。见 [MediaPipe Face Detector](https://ai.google.dev/edge/mediapipe/solutions/vision/face_detector)。
- `printpdf` 是纯 Rust PDF 库，支持字体、图像、页面操作等模块，适合生成任务 PDF 报告。见 [printpdf docs.rs](https://docs.rs/printpdf/latest/printpdf/)。

## 当前项目基建判断

项目当前为 React + Zustand + Tauri 2 + Rust 后端架构。

已有可复用能力：

- RAW 支持集中在 `src-tauri/src/formats.rs`、`raw_processing.rs`、`image_loader.rs`。
- 图像分析已有 `src-tauri/src/culling.rs`，包含清晰度、曝光、感知哈希、相似组雏形。
- ONNX Runtime 已通过 `ort` 接入，AI 模型加载逻辑存在于 `ai_processing.rs`。
- 后端已使用 `rayon` 做并行处理。
- `.rrdata` 元数据由 `ImageMetadata` 统一读写，现有字段为 `version`、`rating`、`adjustments`、`tags`。
- 前端已有 `src/features/focus-areas/` 和 `src-tauri/src/features/focus_areas/`，可作为二开 feature 独立维护样板。

现有不足：

- `src/features/contracts.ts` 目前主要服务 Editor 插槽，不足以挂载 Library 入口、复核页、缩略图徽标和筛选扩展。
- `ImageMetadata` 尚无通用扩展字段，如果智能选图直接写未知字段，后续上游写回 metadata 时可能丢失智能选图数据。
- 现有 `culling.rs` 可复用思想，但当前包含删除/拒绝等逻辑，不满足本次智能选图“不删除、用户确认、可撤销、写 `.rrdata`”的产品约束。

## 总体架构决策

### 核心结论

V1 采用 `Rust/Tauri 原生流水线 + ONNX Runtime 本地模型 + 独立 feature UI`。

不采用：

- Python sidecar: 增加分发、权限、进程管理和跨平台维护成本。
- 云推理: 违反完全离线要求。
- 全量大模型一次性判断: 性能不可控，也难以解释和撤销。
- 直接复用现有 `culling.rs` 作为业务主体: 现有语义与智能选图需求不一致，应只迁移可复用算法思想。

### 模块边界

前端新增：

```text
src/features/smart-culling/
  constants.ts
  contracts.ts
  feature.tsx
  index.ts
  types.ts
  SmartCullingEntry.tsx
  SmartCullingDialog.tsx
  SmartCullingReviewPage.tsx
  SmartCullingReportPage.tsx
  SmartCullingTaskStatus.tsx
  SmartCullingThumbnailBadge.tsx
  useSmartCulling.ts
  useSmartCullingEvents.ts
```

Rust 新增：

```text
src-tauri/src/features/smart_culling/
  mod.rs
  commands.rs
  models.rs
  manifest.rs
  task.rs
  asset_resolver.rs
  analysis_image.rs
  quality.rs
  similarity.rs
  face.rs
  aesthetics.rs
  scoring.rs
  persistence.rs
  report.rs
  undo.rs
  errors.rs
```

只允许改动的上游/通用接入点：

- `src/features/contracts.ts`: 增加通用 Library feature slot。
- `src/features/appFeatures.ts`: 注册 `useSmartCullingFeature()`。
- `src/components/views/LibraryView.tsx`: 渲染通用 feature view slot，不写智能选图业务。
- `src/components/panel/MainLibrary.tsx`: 渲染通用 header action slot，不写智能选图业务。
- `src/components/ui/AppProperties.tsx`: 只增加 command 名称和通用类型，不写业务。
- `src-tauri/src/features/mod.rs`: 统一导出 smart culling commands。
- `src-tauri/src/lib.rs`: 只注册 command 和 `SmartCullingState`，不写业务。
- `src-tauri/src/image_processing.rs`: 增加通用 `featureData` 字段，保证 `.rrdata` 扩展字段不会被上游写回丢失。

## Feature Slot 设计

需要把现有 feature 契约从 Editor 扩展到 Library。

建议在 `src/features/contracts.ts` 增加：

```ts
export interface LibraryFeatureContext {
  currentFolderPath: string | null;
  imageList: ImageFile[];
  selectedPaths: string[];
}

export interface LibraryHeaderActionSlotProps extends LibraryFeatureContext {}

export interface LibraryFeatureViewSlotProps extends LibraryFeatureContext {
  onBackToLibrary(): void;
}

export interface LibraryThumbnailBadgeSlotProps {
  image: ImageFile;
}

export interface LibraryFilterContributor {
  id: string;
  label: string;
  predicate(image: ImageFile): boolean;
}

export interface LibraryFeatureSlots {
  headerActions?: Array<ComponentType<LibraryHeaderActionSlotProps>>;
  views?: Record<string, ComponentType<LibraryFeatureViewSlotProps>>;
  thumbnailBadges?: Array<ComponentType<LibraryThumbnailBadgeSlotProps>>;
  filterContributors?: LibraryFilterContributor[];
}

export interface AppFeatureRegistration {
  editor?: EditorFeatureSlots;
  library?: LibraryFeatureSlots;
  keyboardActions?: Record<string, KeybindHandler>;
}
```

智能选图只在 `src/features/smart-culling/feature.tsx` 注册：

- Library header action: `SmartCullingEntry`
- Library view: `smart-culling-review`、`smart-culling-report`
- Thumbnail badge: 人工优先下的智能建议徽标
- Filter contributors: 智能精选、智能待确认、智能淘汰建议、有智能建议、未经智能选图处理、原因筛选

## 后端 Command 设计

命名统一使用 `smart_culling_*`：

```text
smart_culling_check_models
smart_culling_open_models_dir
smart_culling_load_presets
smart_culling_save_preset
smart_culling_delete_preset
smart_culling_list_recent_tasks
smart_culling_start_task
smart_culling_cancel_task
smart_culling_get_task_result
smart_culling_apply_task_result
smart_culling_discard_task_result
smart_culling_undo_last_task
smart_culling_export_report_pdf
```

事件统一带 `taskId`：

```text
smart-culling:started
smart-culling:progress
smart-culling:review-ready
smart-culling:applied
smart-culling:failed
smart-culling:cancelled
```

Tauri 文档推荐 Channel 处理流式数据，但本项目现有进度通信已经大量使用事件。V1 继续使用事件发送小体量进度和状态，大体量结果通过 command 按 `taskId` 拉取，避免事件 payload 过大。

## 数据持久化设计

### `.rrdata` 扩展

必须给 `ImageMetadata` 增加通用扩展字段：

```rust
#[serde(default, rename = "featureData", skip_serializing_if = "Option::is_none")]
pub feature_data: Option<serde_json::Value>,
```

智能选图写入位置：

```json
{
  "featureData": {
    "smartCulling": {
      "schemaVersion": 1,
      "taskId": "uuid",
      "source": "smart_culling",
      "status": "selected | review | reject_suggestion | unprocessed",
      "rating": 4,
      "appliedRating": 4,
      "confidence": 0.87,
      "degraded": false,
      "reasonCodes": ["sharp", "best_in_group"],
      "reasonText": "主体清晰，组内最佳，曝光正常",
      "groupId": "uuid",
      "groupRank": 1,
      "groupSize": 8,
      "createdAt": "2026-05-12T12:00:00+08:00",
      "appliedAt": "2026-05-12T12:03:00+08:00"
    }
  }
}
```

顶层 `rating` 写入规则：

- 如果顶层 `rating > 0` 且没有 `featureData.smartCulling.appliedRating` 匹配，视为人工评分，跳过。
- 如果顶层 `rating == 0`，用户确认应用后可以写入智能评分，并记录 `appliedRating`。
- 如果顶层 `rating` 与上一次智能写入的 `appliedRating` 一致，允许本功能撤销或覆盖自己的系统结果。
- 如果用户在任务后手动改了顶层 `rating`，后续撤销不得改动该人工结果。

为提高人工识别可靠性，建议在现有 `set_rating_for_paths` 中增加通用来源标记：

```json
{
  "featureData": {
    "ratingSource": {
      "source": "manual",
      "updatedAt": "..."
    }
  }
}
```

该改动是通用元数据来源标记，不包含智能选图业务逻辑。

### 任务级数据

```text
<app_data_dir>/
  smart-culling-presets.json
  models/
    clip_model.onnx
    clip_tokenizer.json
    smart-culling/
      manifest.json
      image_encoder.onnx
      aesthetic_head.onnx
      face_detector.onnx
      face_landmark.onnx
      expression.onnx
  smart-culling/
    tasks/
      <taskId>/
        task.json
        report.pdf
        thumbnails/
```

任务历史保留最近 10 次。撤销后的任务保留 `task.json` 和 `report.pdf`，状态标记为 `revoked`。

## 模型 Manifest

V1 允许智能选图触发下载上游 RapidRAW 已有的 CLIP ONNX 模型，并复用 `<app_data_dir>/models/` 缓存，避免重复下载与模型分叉。

智能选图专用模型仍通过 manifest 管理。原因：

- 满足完全离线。
- 对人脸/表情、专用美学 head 等模型保持 license 审查边界。
- 避免把 2GB 模型纳入安装包。
- 支持后续替换不同模型。

建议 manifest：

```json
{
  "schemaVersion": 1,
  "packageName": "smart-culling-default",
  "packageVersion": "2026.05",
  "models": [
    {
      "role": "image_encoder",
      "file": "image_encoder.onnx",
      "required": true,
      "inputSize": 224,
      "embeddingDim": 512,
      "sha256": "..."
    },
    {
      "role": "aesthetic_head",
      "file": "aesthetic_head.onnx",
      "required": true,
      "dependsOn": "image_encoder",
      "sha256": "..."
    },
    {
      "role": "face_detector",
      "file": "face_detector.onnx",
      "required": false,
      "inputSize": 640,
      "sha256": "..."
    },
    {
      "role": "face_landmark",
      "file": "face_landmark.onnx",
      "required": false,
      "inputSize": 112,
      "sha256": "..."
    },
    {
      "role": "expression",
      "file": "expression.onnx",
      "required": false,
      "inputSize": 224,
      "sha256": "..."
    }
  ]
}
```

缺失处理：

- `image_encoder` 或 `aesthetic_head` 缺失: 禁止开始基础智能选图。
- 人像表情总开关关闭: face 系列模型缺失不影响任务。
- 人像表情总开关开启但 face 模型缺失: 允许降级运行，但必须提示并标记 `可信度降低`。
- 选图模式为人像/婚礼/儿童且 face 模型缺失: 强提示，默认建议用户取消或关闭人像表情分析。

## 模型选型

### 推理运行时

首选：ONNX Runtime + `ort`。

理由：

- 项目已经接入 `ort`，依赖成本最低。
- 同一模型格式可覆盖 macOS、Windows、Linux。
- Execution Provider 可按平台启用 CoreML、DirectML、CUDA，并回退 CPU。
- Rust 后端可直接控制任务队列、取消、事务写入和报告生成。

运行策略：

```text
macOS: CoreML EP -> CPU
Windows: DirectML EP -> CUDA EP(可选) -> CPU
Linux: CUDA EP(可选) -> CPU
```

实际可用 EP 必须运行时探测。若当前打包的 `libonnxruntime` 不包含某 EP，不得崩溃，必须回退 CPU 并在模型状态中展示。

### 美学评分

首选：OpenCLIP/CLIP 图像 encoder + LAION-style linear aesthetic head。

理由：

- 图像 embedding 可同时复用给美学评分、相似图二次确认、用户偏好评分。
- 美学 head 很小，推理成本低。
- LAION aesthetic predictor 证明了 `CLIP embedding + 线性 head` 的可行性。

备选：NIMA MobileNet/EfficientNet ONNX。

适用场景：

- 如果 CLIP encoder 太慢，NIMA-style 小模型可作为低配设备 fallback。
- 如果用户只需要基础质量评分，不需要 CLIP 语义偏好，可用 NIMA-style 模型减少体积。

### 相似/重复

首选两阶段策略：

1. `image_hasher` DoubleGradient 感知哈希做快速候选分组。
2. CLIP embedding cosine similarity 对候选组做二次确认，降低误把相似色块/构图误分为同组的风险。

1000 张以内不需要引入向量数据库或 HNSW。感知哈希 O(N²) 距离比较约 100 万次，成本很低。后续如果支持上万张跨库历史分组，再评估 HNSW/SQLite 向量扩展。

### 清晰度与曝光

首选传统 CV 指标 + 局部加权：

- Laplacian variance / Tenengrad 判断锐度。
- 人像模式对人脸、眼睛、主体 crop 加权。
- 风光模式对整体画面和关键区域加权。
- 曝光用灰度直方图、暗部/高光裁切比例、中间调占比。

理由：

- 无模型依赖，速度快，可解释。
- 与照片初筛的核心废片判断高度相关。
- 适合在 RAW 分析图上批量并行执行。

### 人脸、人像表情

V1 建议支持可插拔模型适配，不默认绑定某一个模型来源。

推荐适配优先级：

1. SCRFD KPS ONNX: 适合通用摄影场景，检测质量好，支持关键点；但 InsightFace 模型仅非商业研究用途，不能默认分发。
2. BlazeFace full-range: 轻量，官方文档面向移动/本地推理，适合作为轻量 detector 路线；如转 ONNX 使用，需要单独验证精度与 license。

表情判定策略：

- 闭眼: 优先通过 landmark 计算眼睛纵横比；没有 landmark 时降级为眼部 crop 分类器。
- 糊脸: 对 face crop / eye crop 做局部清晰度。
- 主体是否看镜头: 基于关键点姿态和瞳孔/眼部方向做启发式判断；V1 可标记低置信度。
- 微笑/表情异常: 使用可选 expression model；缺失时不强行判断。
- 多人合照最佳表情: 对每个人脸给出表情可用性分，再按主体大小、居中度、清晰度加权。

## RAW 分析流水线

```text
选择范围
  -> 资产归一化
  -> 跳过规则
  -> RAW 分析图生成
  -> 快速指标
  -> 相似分组
  -> 模型批处理
  -> 模式化综合评分
  -> 复核结果
  -> 用户确认
  -> 事务写入 .rrdata + 任务报告
```

### 资产归一化

输入照片统一转为 `SmartCullingAsset`：

```rust
struct SmartCullingAsset {
    asset_id: String,
    primary_path: PathBuf,
    display_path: String,
    sidecar_path: PathBuf,
    raw_path: Option<PathBuf>,
    jpeg_pair_path: Option<PathBuf>,
    is_virtual_copy: bool,
    is_raw: bool,
    existing_rating: u8,
    is_edited: bool,
    skip_reason: Option<SkipReason>,
}
```

规则：

- 当前文件夹全部图片为默认范围。
- RAW + JPEG 同名视为一个资产，优先 RAW。
- 虚拟副本不单独分析。
- 已有人工评分跳过。
- 已有用户修图调整默认跳过。
- 只处理项目支持的 RAW；非 RAW 作为同名 JPEG 附属或跳过。

### 分析图生成

为性能和稳定性，不对全分辨率 RAW 直接跑所有模型。

建议生成三档图：

- `fast`: 长边 720，用于感知哈希、曝光、粗清晰度。
- `global`: 长边 1536，用于美学、CLIP embedding、主体/整体判断。
- `detail`: 长边 2048 或局部 crop，用于人脸、眼睛、组内最佳复判。

实现优先级：

1. 优先尝试 RAW 内嵌预览或现有快速解码路径。
2. 预览图不足时使用 `fast_demosaic`。
3. 只有候选最佳图、待确认图、人像局部图进入更高分辨率精判。

### 批处理与并发

- RAW 解码与传统指标使用 `rayon` 并行。
- ONNX 推理使用单独 `InferencePool`，按模型维持 session。
- 同时运行任务限制为 1 个。
- 取消使用 `AtomicBool` 或 task token，所有阶段都定期检查。
- 模型 batch size 根据设备动态选择：CPU 4-8，GPU/ANE 16-32。
- 图片分析结果先写入任务内存和 task workspace，不直接写 `.rrdata`。

## 评分模型

每张图先得到标准化指标：

```text
sharpness_global
sharpness_subject
sharpness_face
sharpness_eye
exposure
aesthetic
preference_match
duplicate_rank
face_quality
expression_quality
gaze_quality
confidence
```

综合分：

```text
score = modeWeights * normalizedSignals - penalties + bonuses
```

输出映射：

```text
>= 0.82 -> 5 星 / 精选
>= 0.68 -> 4 星 / 精选
>= 0.50 -> 3 星 / 待确认
>= 0.32 -> 2 星 / 待确认
<  0.32 -> 1 星 / 淘汰建议
```

阈值 V1 固定，不暴露给用户。严格/均衡/宽松预设通过权重和惩罚强度影响分数，不直接让用户编辑阈值。

原因生成必须来自规则和指标，不由大语言模型生成。每张图保存 `reasonCodes`，UI/PDF 使用中文模板渲染。

## 复核与应用

分析完成后自动进入复核页。

复核页数据来源是任务结果，不是立即写入后的 Library 状态。

用户确认后执行：

1. 加载每张照片当前 `.rrdata`。
2. 再次检查人工评分/人工修改保护。
3. 生成变更快照。
4. 写临时文件 `<sidecar>.tmp`。
5. 原子替换 `.rrdata`。
6. 写任务状态为 `applied`。
7. 生成或更新 PDF 报告。

如果任意关键写入失败：

- 已写入部分必须根据快照回滚。
- 任务状态标记为 `failed`。
- 生成失败报告。

## 一键撤销

撤销只处理最近一次已应用任务。

撤销依据：

- task workspace 中的变更快照。
- `.rrdata featureData.smartCulling.taskId` 与任务 ID 匹配。
- 顶层 `rating` 仍等于该任务写入的 `appliedRating`。

若任务后用户手动改了星级或颜色标签，撤销不得覆盖用户修改，只撤销仍可确认属于本任务的字段。

撤销后：

- 任务状态改为 `revoked`。
- 历史报告保留并标记已撤销。
- PDF 重新导出时显示已撤销状态。

## PDF 报告

V1 推荐使用 Rust `printpdf` 生成 PDF。

理由：

- 不依赖浏览器打印或系统打印服务。
- 后端已有任务数据和缩略图路径，生成报告更直接。
- 纯 Rust 便于跨平台分发。

要求：

- 内置或要求用户安装可嵌入的中文字体，建议使用 Noto Sans CJK SC。
- 摘要页不放缩略图。
- 明细页可选小缩略图；缩略图失败不影响 PDF 导出。
- 报告包含：分析总数、跳过数量、精选、待确认、淘汰建议、失败数量、每张图片名称、星级、原因、跳过/失败原因、任务状态。

## 性能策略

目标：1000 张 RAW，中端设备以上尽量 2 分钟内。

关键优化：

- 只解码分析所需尺寸，不做全分辨率完整渲染。
- 快速阶段覆盖全部图片，精判阶段只处理候选和疑难图。
- 感知哈希先分组，避免对所有图做昂贵模型二次比较。
- CLIP embedding 一次生成，多处复用。
- 人脸/表情模型只在人像相关模式或总开关启用时运行。
- 模型 session 常驻，任务间复用。
- CoreML/DirectML/CUDA 可用则启用，不可用立即 CPU fallback。
- 任务取消时停止后续队列，不再启动新推理。

性能验收建议：

- 100 张 RAW: 开发回归基准。
- 1000 张 RAW: 发布前性能门槛。
- 至少覆盖 Apple Silicon、Windows 中端独显/核显、Linux CPU fallback 三类环境。

## 安全与可靠性

- 模型不随安装包内置；共享 CLIP 模型仅在用户点击“下载模型”后下载。
- 共享 CLIP 下载优先复用上游下载器；主下载源不可达时，智能选图 feature 可尝试备用镜像，下载后必须校验 sha256。
- 下载失败、模型缺失或初始化失败时必须允许基础模式继续运行，并标记可信度降低。
- `manifest.json` 必须校验文件存在、大小、sha256、role、输入尺寸。
- 不把图片内容、路径、EXIF 发送到网络。
- 任务取消不写 `.rrdata`。
- 任务失败整体不写，部分失败可复核成功项。
- 写 `.rrdata` 必须原子替换。
- 智能选图永不删除照片。
- 所有降级结果必须标记 `可信度降低`。

## 测试计划

### 单元测试

- 资产归一化：RAW+JPEG、虚拟副本、非 RAW、人工评分、已修图。
- `.rrdata` 读写：保留 `featureData`、人工评分保护、撤销保护。
- 分数映射：各模式权重、严格/均衡/宽松预设。
- 相似分组：pHash 阈值、组内最佳排序、每组保留 N。
- 原因模板：reasonCodes 到中文说明。

### 集成测试

- `smart_culling_start_task -> review -> apply -> undo` 完整流程。
- 模型缺失、非关键模型降级、关键模型缺失。
- 取消任务不落盘。
- PDF 导出和历史任务保留 10 次。
- Library 筛选和缩略图智能徽标。

### 评测集

仓库只保存评测协议，不提交真实 RAW。

建议新增：

```text
docs/smart-culling-evaluation.md
fixtures/smart-culling/.gitkeep
```

评测指标：

- 与摄影师人工精选的一致率。
- 淘汰建议误伤率。
- 相似组最佳命中率。
- 人像闭眼/糊脸识别准确率。
- 1000 张 RAW 总耗时。
- CPU/GPU 内存峰值。

## 实施阶段

### 阶段 1: 基建与契约

- 增加 Library feature slot。
- 新建前后端 smart-culling feature 目录。
- 增加 `featureData` 通用 metadata 字段。
- 实现模型目录、manifest 校验、共享 CLIP 下载入口、预设读写。

### 阶段 2: 无模型基础流水线

- 资产归一化。
- RAW 分析图生成。
- 清晰度、曝光、pHash、相似分组。
- 复核页基础版。
- 预览模式。

### 阶段 3: ONNX 推理接入

- `InferencePool` 与 model registry。

### 已落地: 产品闭环

当前实现已将智能选图从基础分析推进到可完成一次真实工作流的产品闭环：

- 任务结果写入 `<app_data_dir>/smart-culling/tasks/<taskId>/task.json`，最近历史保留 10 次。
- 应用结果时生成 `applied_snapshots.json`，用于撤销恢复应用前 `.rrdata`。
- PDF 报告由 Rust 本地生成，使用系统字体渲染中文页面，再嵌入 PDF，避免依赖云服务。
- 复核页支持状态、原因、星级、颜色维度查看；相似组默认折叠，只显示最优。
- Library 视图支持智能选图状态和相似组筛选，筛选判断由 smart-culling feature 通过通用 `filterGroups` 注入。
- 配置弹窗支持用户保存、加载、删除个人选图策略预设，预设持久化到 `<app_data_dir>/smart-culling/presets.json`。
- 任务运行中支持取消，取消信号由后端 `AtomicBool` 检查，不写 `.rrdata`。
- 预览模式在前后端均禁止应用写入，应用结果只按后端返回的实际写入路径更新图库状态。
- 颜色标签复用上游 `color:` 标签机制，智能选图只作为用户复核维度，不赋予固定含义。
- 美学偏好与人像检查项通过本地规则和 CLIP 语义辅助参与评分；专用美学 head、人脸检测、表情模型仍作为可选模型位保留。
- image encoder + aesthetic head。
- face detector / landmark / expression 适配。
- 降级运行与可信度标记。

### 阶段 4: 应用、撤销、报告与 UI 完整化

- 事务写 `.rrdata`。
- 一键撤销。
- PDF 报告。
- 最近 10 次历史任务。
- Library 筛选与徽标。
- 用户配置预设保存/加载/删除。
- 运行中取消任务。

### 阶段 5: 性能与评测

- 100/1000 张 RAW 性能基准。
- 样片金标准评测。
- 模型/权重调优。
- 非侵入扫描。

## 最终选型清单

| 领域       | V1 选型                                                         | 说明                            |
| ---------- | --------------------------------------------------------------- | ------------------------------- |
| 应用架构   | Tauri 2 + Rust feature module + React feature UI                | 沿用项目现有基建                |
| 推理运行时 | ONNX Runtime via `ort`                                          | 已存在依赖，支持 EP 回退        |
| 硬件加速   | CoreML / DirectML / CUDA 可用则启用，CPU fallback               | 运行时探测，不强依赖            |
| RAW 解码   | 复用现有 `raw_processing` / `image_loader`，新增分析图 provider | 避免重复造 RAW 管线             |
| 相似分组   | `image_hasher` pHash + CLIP cosine 二次确认                     | 快速、可解释、低依赖            |
| 美学评分   | CLIP/OpenCLIP image encoder + LAION-style aesthetic head        | embedding 可复用                |
| 质量评分   | Laplacian/Tenengrad/曝光直方图 + 模式权重                       | 高速、可解释                    |
| 人脸检测   | 插件式 SCRFD/BlazeFace ONNX adapter                             | 不默认分发有 license 风险的模型 |
| 表情判断   | landmark 规则 + 可选 expression model                           | 允许降级                        |
| 持久化     | `.rrdata featureData.smartCulling` + task workspace             | 可追踪、可撤销                  |
| PDF        | `printpdf` + 中文字体                                           | 后端纯 Rust 生成                |
| 通知       | Tauri notification plugin                                       | 仅系统通知，不播放声音          |

## 风险与约束

- InsightFace 模型 license 仅非商业研究用途，不能默认打包或自动下载。
- CoreML/DirectML/CUDA 是否可用取决于实际打包的 ONNX Runtime 动态库，必须运行时探测。
- 1000 RAW / 2 分钟依赖分析图生成效率，必须优先实现嵌入预览/快速解码策略。
- 人像“主体是否看镜头”和“表情异常”天然有主观性，V1 必须展示置信度和原因，不能做绝对判定。
- `.rrdata featureData` 是保障数据不丢失的基础，必须先完成再做智能写入。
