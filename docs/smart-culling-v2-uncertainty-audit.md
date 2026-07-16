# 智能选图 V2 开发前不确定性审计

> 版本: 1.0
> 日期: 2026-07-15
> 审计基线: `06405ea2aaa22ebf87288bc0c88cef5c202d7b2a`
> 目的: 在修改生产代码前，记录最没有把握的事项、证据、剩余未知和处置门槛

## 1. 最没有把握的事项

最不确定的不是“能否画出复核页”，而是以下组合约束能否同时成立：

> 分析必须使用包含调色、裁剪、蒙版、修复和 LUT 的当前渲染状态；同时保持智能选图业务完全位于
> feature 内，不复制上游渲染算法，并让 1000 张任务尽量在目标低配支持设备上 5 分钟完成。

它同时涉及正确性、性能、GPU 并发、上游隔离和设备门槛。如果路径选错，会出现三种严重结果：分析
画面与用户看到的不一致、为了正确画面逐张 JPEG/IPC 导致性能不可接受、或把大量业务侵入上游
`lib.rs`/渲染模块。

## 2. 已调查证据

### 2.1 当前 V1 不满足要求

`src-tauri/src/features/smart_culling/mod.rs::analyze_single_image` 调用
`load_base_image_from_bytes(..., true, ...)`，随后直接分析基础图。它没有加载 `.rrdata.adjustments`，也
没有进入裁剪、几何、蒙版、LUT 和最终 GPU 渲染，因此不能复用为 V2 输入。

### 2.2 现有预览命令画面正确但调用形态不合适

`src-tauri/src/lib.rs::generate_preview_for_path` 已按正确顺序应用现有调整和 GPU 渲染，但最后把每张
图片编码为质量 92 的 JPEG，并通过 Tauri IPC 返回前端。千张循环会引入不必要的编码、内存复制、
消息和前端调度成本；它适合交互预览，不适合批量模型输入。

### 2.3 底层渲染能力可以在 feature 内复用

当前源码确认以下函数已公开到 crate 内：

- `image_loader::load_and_composite`
- `adjustment_utils::apply_all_transformations`
- `mask_generation::generate_mask_bitmap`
- `mask_generation::resolve_warped_image_for_masks`
- `image_processing::get_all_adjustments_from_json`
- `image_processing::resolve_tonemapper_override`
- `lut_processing::get_or_load_lut`
- `gpu_processing::get_or_init_gpu_context`
- `gpu_processing::process_and_get_dynamic_image`
- `file_management::parse_virtual_path` / `read_file_mapped`

因此可以在 `src-tauri/src/features/smart_culling/infrastructure/render_input.rs` 组合现有管线，直接在
Rust 内得到 `DynamicImage` 或模型张量；不需要复制渲染算法、不需要新增前端像素协议，也不需要在
`src-tauri/src/lib.rs` 增加智能选图业务。

### 2.4 已识别的正确性陷阱

- 现有 GPU 处理在图片超过 `max_texture_dimension_2d` 时会记录 warning 并返回未处理基础图；V2 必须
  把它提升为明确失败或精确分块方案，不能接受静默画面不一致。
- `ImageMetadata.adjustments` 在任务运行中可能变化；分析期禁止编辑，确认时仍须比较 sidecar 基线。
- 共享 GPU processor 有 mutex；不能用 Rayon 无界并发同时提交渲染。
- RAW 开发、几何、mask 和 LUT 的阶段顺序必须与现有预览一致，不能只调用最终 shader。
- 当前仓库没有可代表 1000 张真实 RAW/JPEG 混合任务的合法性能数据集，不能在开发前宣称 5 分钟
  已通过。

## 3. 处理方案

### 3.1 架构选择

采用“feature-local RenderInputAdapter + 现有公开渲染函数”的方案：

1. Rust 任务清单记录每个资产的 adjustments 摘要和 sidecar 基线。
2. RenderInputAdapter 在受控队列中读取、解码并复用完整现有渲染顺序。
3. 渲染结果在 Rust 内缩放/归一化，直接交给推理适配器。
4. 快速阶段使用受控尺寸；只有候选进入高分辨率精查。
5. 解码可并行，GPU 提交串行或小窗口背压；并发数由基准决定，不硬编码为 CPU 核数。
6. 任何无法保证当前画面一致的路径返回稳定错误码并列为失败/跳过。

### 3.2 必须先通过的刺探

- 调整、裁剪、蒙版、修复和 LUT 五类夹具与现有预览做像素或感知一致性比较。
- 记录 JPEG/IPC 预览路径与 feature-local 内存路径的阶段耗时，证明选择没有额外往返。
- 在 macOS/Windows 目标设备记录 GPU adapter、纹理上限、渲染吞吐和共享队列影响。
- 对真实 RAW/JPEG 混合目录逐步跑 50/200/1000 张，记录冷/热缓存和精查比例。
- 超纹理上限、GPU 初始化失败、LUT 丢失和取消均有确定性测试。

## 4. 置信度

这里区分“能够安全处理该不确定性”和“产品指标已经验证”：

| 判断 | 置信度 | 依据 |
| --- | ---: | --- |
| 能在 feature 内取得与当前预览一致的渲染画面 | 95% | 所需底层函数均已公开，现有命令给出完整可复用顺序 |
| 能避免逐张 JPEG/IPC 和上游业务侵入 | 98% | 可直接在 Rust feature 内得到图片/张量；现有 Library slot 足够 |
| 能建立可测、可取消、带背压的批量输入端口 | 93% | 已有 GPU context、任务状态可独立封装；共享 mutex 风险可被基准约束 |
| 能处理“5 分钟未达标”而不污染功能或静默降级 | 96% | 性能被独立 Gate 阻断；不支持时只禁用 feature |
| 当前就能证明目标低配设备 1000 张在 5 分钟内 | 35% | 缺目标设备矩阵、真实混合数据集、GPU 推理 provider 和完整模型基准 |

对“能否以正确工程路径处理最不确定事项”的综合置信度为 **94%**，已超过开始隔离式代码工作的
90% 门槛。对“5 分钟指标已经达成”的置信度仍然不足，因此该结论绝不能写成已完成能力；它继续由
`P2-15` 至 `P2-18` 和 G2 阻断发布。

## 5. 生产代码准入结论

可以开始的代码：

- 当前 HEAD 的所有权/V1 清单验证工具。
- V2 领域契约、状态机、扫描、配对、人工保护和安全持久化测试。
- RenderInputAdapter 的最小刺探和一致性测试。

尚不可宣称完成或默认启用的代码：

- 完整 macOS/Windows GPU 支持设备矩阵。
- 1000 张/5 分钟性能承诺。
- 九模式、关键人物、组合识别和故事线效果。
- 未完成许可、shape、算子覆盖和真实摄影验证的模型集。

任何刺探失败都回到本记录更新证据和方案，不在上游文件里追加例外分支，也不以 CPU 或基础图静默
降级掩盖失败。

## 6. 首次代码刺探结果

已在 `src-tauri/src/features/smart_culling/infrastructure/render_input.rs` 实现最小输入适配器，并通过
当前 crate 编译。它直接复用第 2.3 节列出的现有函数，读取 `.rrdata.adjustments`、补水缓存数据、
应用补丁/几何/蒙版/LUT 和 GPU 渲染，最终返回内存中的 `DynamicImage`，没有 JPEG 编码或前端 IPC。

同时增加了对 `max_texture_dimension_2d` 的显式检查，避免落入宿主 GPU 函数“超限后返回未处理基础图”
的静默旁路。当前定向测试共 25 项通过，其中 2 项覆盖纹理上限；真实调整画面一致性和目标设备吞吐仍按
P2/G2 保持未验证。

## 7. Apple M4 Max Core ML 刺探与修复

2026-07-15 在本机 Apple M4 Max（32 核 GPU、36GB、macOS 26.5.2）完成生产运行时刺探：

- 原配置使用 Core ML 默认 `NeuralNetwork` 格式，并对所有模型强制静态输入。YuNet 可完整进入
  Core ML，但 OCEC 的动态 `batch` 维度导致图节点留在默认 CPU EP，因此被严格门槛正确拒绝。
- 改用 Core ML `MLProgram`，并通过 ONNX Runtime 维度覆盖把 OCEC 的 `batch` 固定为实际使用值 1。
  两个模型随后均能在 `session.disable_cpu_ep_fallback=1` 下创建会话并完成真实推理。
- Core ML `ProfileComputePlan` 诊断显示主要模型算子调度到 Apple M4 Max 的 `MLGPUComputeDevice`；
  没有通过移除 CPU 回退禁令或隐藏降级来让预检通过。
- 两个编译会话改为应用生命周期缓存，避免重复进入功能时重新编译，并规避 Core ML 原生异步清理
  与后续任务竞争。
- 新 Debug `RapidRAW.app` 已在真实桌面进程中进入智能选图复核页并显示 17 张分析结果；原设备不支持页
  不再出现。Smart Culling 定向测试更新为 42 项通过。

当前设备的推理路径由“未验证/拒绝”更新为“已验证”。Windows DirectML、其他 Apple Silicon 型号、
1000 张性能和真实摄影效果仍由 P2/G2 阻断发布。
