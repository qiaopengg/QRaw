# 智能选图 V2 技术架构

> 版本: 2.0
> 日期: 2026-07-15
> 状态: 与产品需求 3.0 对齐；发布能力由 Gate 证据决定
> 产品基线: `docs/smart-culling-v2-requirements.md`
> 执行清单: `docs/smart-culling-v2-implementation-plan.md`
> 不确定性记录: `docs/smart-culling-v2-uncertainty-audit.md`

## 0. 架构结论

智能选图必须以独立 feature 交付，前后端业务代码分别只放在：

- `src/features/smart-culling/**`
- `src-tauri/src/features/smart_culling/**`

宿主层只保留已有通用插槽和一个稳定 Tauri 网关。不得把任务状态、评分、模型、配对、保护、复核或
持久化业务继续写进 `App.tsx`、Library 组件、通用 hooks、`src-tauri/src/lib.rs` 或其他上游模块。

本架构采用以下硬边界：

1. 当前 V1 智能选图只作为退役清单，不复用其评分与写入业务。
2. 复用 QRaw 已有文件识别、RAW 解码、调整、蒙版、LUT、GPU 渲染、`.rrdata` 和 Library feature slot。
3. 所有新状态、契约、策略、任务编排、UI 和测试均归 feature 自己所有。
4. 模型随安装包交付；运行时不下载模型、不请求云端、不接外部模型服务。
5. 生产路径不静默降级。GPU、运行时或模型不满足契约时只禁用智能选图。
6. 分析和复核阶段只写 feature 临时区；用户确认时才以乐观并发方式合并 `.rrdata`。
7. 不删除、移动或复制照片，不提供撤销、任务历史、PDF 或偏好学习。

## 1. 当前源码事实与复用边界

以下事实基于当前分支 HEAD `06405ea2aaa22ebf87288bc0c88cef5c202d7b2a`，不是对未来实现的假设。

| 能力 | 当前实现 | V2 决策 |
| --- | --- | --- |
| Library 扩展 | `src/features/contracts.ts` 已有 header、badge、view、filter slot | 直接复用，冻结宿主 slot |
| feature 注册 | `src/features/appFeatures.ts` 已注册智能选图 | 保留注册点，不加入业务判断 |
| 图片格式 | `src-tauri/src/formats.rs` 提供 RAW/非 RAW 判断 | 复用后由 feature 排除 GIF/TIFF/TIF |
| sidecar 定位 | `parse_virtual_path`、`get_primary_sidecar_path` | 复用，不自行拼路径 |
| sidecar 数据 | `ImageMetadata { rating, tags, adjustments, featureData }` | 复用结构，feature 负责安全合并 |
| 当前渲染 | `load_and_composite`、`apply_all_transformations`、蒙版、LUT、`process_and_get_dynamic_image` 均可复用 | feature 内组合无 JPEG/IPC 的分析输入端口 |
| GPU 初始化 | `get_or_init_gpu_context` | 复用；另加 feature 自有设备预检和推理能力检查 |
| 颜色标签 | `color:<name>`，已有 red/yellow/green | 复用既有语义和显示 |
| V1 模型 | YuNet 与 OCEC 模型已随 `resources` 打包 | 只作候选资产；未通过效果/许可 Gate 前不视为 V2 模型集 |

当前 V1 的基础图分析、pHash 分组、固定质量权重、删除/零星/红标操作和直接 `fs::write` 均不满足
V2；这些代码不能继续扩建。V2 可复用的是宿主能力，不是 V1 业务结果。

## 2. 目标目录与责任

### 2.1 前端

```text
src/features/smart-culling/
  feature.tsx              # 只注册已有 Library 插槽
  gateway.ts               # 唯一 invoke/event 适配层
  contracts.ts             # 前后端 DTO；不承载领域行为
  i18n.ts                  # zh-CN / en + 其他语言回退 en
  state/
    taskStore.ts            # 单任务状态机
  components/
    entry/
    setup/
    progress/
    review/
    result/
    badges/
  selectors/               # 纯派生状态
  __tests__/
```

`feature.tsx` 保持注册器职责，不把任务状态和页面实现堆进单个大文件。复核页按区域拆分，业务状态不
散落到卡片组件。React 数据获取和事件订阅集中在 gateway/store 层，避免每张缩略图各自 invoke 或监听。

### 2.2 Rust

```text
src-tauri/src/features/smart_culling/
  mod.rs                    # 唯一宿主网关和模块导出
  application/
    coordinator.rs          # 单任务编排、取消、阶段与事件
    commands.rs             # 请求路由
  domain/
    asset.rs                # 资产、配对、人工保护
    task.rs                 # 状态机和错误码
    result.rs               # 星级、标签、来源、原因
    policy.rs               # 确定性映射和保护规则
  infrastructure/
    catalog.rs              # 递归扫描、格式过滤、资产配对
    render_input.rs         # 当前渲染状态输入端口
    inference.rs            # 本地模型运行时适配
    persistence.rs          # 基线、合并、原子替换
    model_bundle.rs         # 安装包模型验证
    temporary.rs            # 当前任务临时结果与清理
  tests/
```

模块按需求实际出现时建立，不一次性生成空壳。任何超过 500 行或同时承担网关、领域、推理、写入两项
以上职责的文件必须拆分。

## 3. 单一宿主网关

宿主最终只注册一个智能选图命令：

```text
smart_culling(request) -> response
```

请求以带版本和 `kind` 的枚举路由，例如预检、启动、取消、查询、复核修改、确认、放弃和失败重试。
事件统一为一个版本化 envelope：

```text
smart-culling://event
```

这样新增页面或阶段无需持续修改 `src-tauri/src/lib.rs`。网关只负责反序列化和调用 feature application
层，不包含业务分支。当前两个 V1 命令在 V2 数据闭环建立后退役。

## 4. 任务状态机

全局只允许一个活动任务：

```text
Idle
  -> Preflighting
  -> Configuring
  -> Indexing
  -> Rendering
  -> Analyzing
  -> Organizing
  -> ReadyForReview
  -> Confirming
  -> Completed
```

旁路状态：

- `Cancelling -> ReadyForReview`：保留已完成结果，未完成保持原样。
- `Abandoning -> Idle`：清除临时结果和人物数据，不写 sidecar。
- `Failed -> ReadyForReview | Idle`：有部分结果则可复核；无可用结果则回到空闲。
- `Unsupported -> Idle`：只禁用智能选图。

进入 `ReadyForReview` 前不写正式 `.rrdata`。任务根目录在启动后冻结，Library 切换文件夹不改变任务
范围。分析期间由 feature 暴露宿主能力锁，禁止编辑与重任务，只保留浏览和切换目录。

## 5. 预检与设备 Gate

预检必须在展示可启动配置前完成：

1. 当前平台为 macOS 或 Windows。
2. QRaw GPU 渲染上下文可初始化，并取得满足图片渲染要求的 adapter/limits。
3. 本平台推理执行器存在，核心模型能在目标 GPU provider 上建立会话。
4. 模型文件存在，SHA、版本、输入输出 shape 和契约匹配。
5. 可用内存、显存估算与临时磁盘满足本任务预算。
6. 当前没有另一个运行中或待复核任务。

GPU-only 在 P0 基准通过前保持“待验证约束”。生产实现不得把 CPU fallback 伪装为支持；CPU 只能
用于开发/基准对照，并由编译或测试开关隔离。预检失败返回稳定错误码和简洁说明，不影响其他功能。

## 6. 目录扫描与资产身份

`catalog` 从用户启动的根目录递归扫描，输出稳定任务清单：

- 只接纳 QRaw 支持且非 GIF/TIFF/TIF 的静态图片。
- 每个子文件夹单独组织相似组和故事段落。
- 同目录、同 stem 的一个 RAW 加一个 JPEG/JPG 可确定配对为同一资产。
- 配对有 RAW 时以 RAW 为分析/写入主资产。
- 同 stem 多 RAW、多 JPEG、导出后缀或其他歧义不猜测，记为跳过。
- 只有非 RAW 时，该文件自身为资产和 sidecar 主体。
- 每个清单项记录路径、大小、mtime、sidecar 路径和 sidecar 基线摘要。

扫描结果区分 `eligible`、`protected`、`skipped` 和 `failed`。正常排除格式属于 skipped，不计失败。

## 7. 人工保护判定

V2 元数据命名空间为：

```text
featureData.smartCullingV2
```

其中至少保存 `schemaVersion`、`source`、`resultId`、`rating`、`colorLabel`、原因、模式、可信度、
模型/策略版本和确认时间。

判定规则：

1. 顶层 `rating > 0` 或存在颜色标签，但没有可验证、且与顶层值一致的 V2 AI 记录：人工结果。
2. V2 `source = manual`：人工结果，包括用户把星级/标签取消到空值。
3. V2 `source = ai` 且顶层星级/颜色仍与记录一致：系统结果，可再次分析。
4. V2 `source = ai` 但顶层值与记录不一致：按人工结果自愈，后续受保护。
5. RAW/JPEG 任一成员人工：整个资产受保护。
6. 旧 `featureData.smartCulling` 不能证明 AI 所有权；默认按历史未知处理，不覆盖已有顶层人工结果。

普通 Library 的星级/标签命令需要通过 feature 无关的来源字段或 feature 自有同步观察，把被改动的 V2
AI 记录转为 manual。具体接入只能使用已拥有的通用元数据事件/slot；若做不到，必须先新增通用来源
契约并单独评审，不能在普通组件里硬编码 `smartCulling`。

## 8. 当前渲染状态输入

V2 不调用现有 `generate_preview_for_path` 做千张循环，因为该命令会把每张图片编码为 JPEG 后再经 IPC
返回前端。分析输入在 Rust feature 内完成，按现有预览顺序复用：

1. `parse_virtual_path` 定位图片和 `.rrdata`。
2. 读取当前 `ImageMetadata.adjustments`。
3. `load_and_composite` 加载 RAW/非 RAW 并合成修复补丁。
4. `apply_all_transformations` 应用裁剪、旋转和几何。
5. 复用现有 mask definition、mask bitmap 和 warped image 解析。
6. 复用 `get_all_adjustments_from_json`、tonemapper 和 LUT 缓存。
7. 调用 `process_and_get_dynamic_image` 得到当前 GPU 渲染结果。
8. 在 Rust 内直接缩放为模型输入，不做 JPEG 编码、不跨 IPC 传像素。

这些函数在当前代码中已公开可复用，因此不需要复制渲染算法，也不需要把智能选图业务放进
`src-tauri/src/lib.rs`。P0 必须用带调色、裁剪、蒙版和修复的样本做像素/感知差异验证，并记录每阶段
耗时。现有 GPU 函数在图片超过最大纹理尺寸时会返回未处理图，V2 不得接受这个静默旁路；预检或
输入端口必须把它转为明确失败/跳过。

## 9. 分层分析流水线

流水线按资源预算分层，效果策略不可由 UI 阈值驱动：

1. `Index`：目录、资产配对、人工保护、基线摘要。
2. `RenderFast`：当前渲染状态的受控短边输入和基础统计。
3. `EmbedAndQuality`：内容向量、技术质量、主体/脸部证据。
4. `Group`：仅在同一子文件夹内做时间/内容相似分组。
5. `RenderFine`：只对边界样本、组内候选和关键人物做更高分辨率精查。
6. `RouteAndScore`：自动或用户指定模式下生成独立维度分。
7. `Recommend`：组内保留 3-5 张，多样性与召回优先。
8. `Explain`：从真实证据用确定性模板生成最多两条原因。
9. `ReviewSnapshot`：写当前任务临时结果，发完成事件。

模型选型、阈值和模式权重必须通过真实摄影数据验证，不能由文档预设为已正确。P1 先完成安全闭环和
可替换的评分端口；P2 再接入达到 Gate 的模型能力；P3 才启用故事工作台与关键人物完整体验。

## 10. 结果语义

领域结果使用强类型，不以自由字符串传播关键状态：

- `Rating`: 1..5
- `ColorLabel`: green | yellow | red
- `Source`: ai | manual
- `Decision`: selected | needs_review | reject_suggestion
- `ReasonCode`: 受控枚举 + 数值证据
- `Confidence`: 0..1
- `Mode`: auto + 九种拍摄类型

颜色和星级分别计算，不能机械互相映射。低可信度、配对歧义、疑似拍摄组合和未验证能力不得标红。
原因文本在前端由本地化模板渲染，后端保存 reason code 与证据，不保存模型自由生成文案。

## 11. 临时数据与取消

任务临时目录只保存完成复核所需的最少数据：任务清单、结果、原因证据、缩略图引用和当前任务人物
特征。不得复制原图。

- 取消令牌在每个重阶段前检查；收到取消后不再调度新解码/渲染/推理。
- 已完成单项进入可复核快照，未完成项保持原样。
- 人物特征在完成、放弃、失败、退出或崩溃清理时删除。
- 下次启动只清理孤儿临时目录，不恢复任务历史。

进度由 `completed/total/currentStage` 和阶段实测移动平均产生，ETA 不得使用固定模拟计时。

## 12. 确认写入与并发安全

每个资产在 Index 阶段记录 sidecar 基线：是否存在、文件大小、mtime 和内容 SHA。确认时逐项执行：

1. 重新读取当前 sidecar 和基线。
2. 若发生变化，重新判断顶层星级、颜色标签、调整和 V2 来源。
3. 若星级/标签变为人工或来源不明，拒绝该项并报告冲突。
4. 调色/裁剪等非保护字段变化不直接拒绝，但结果基于旧画面时必须标记过期，要求重新分析该项。
5. 仅对最终勾选且未冲突资产写入顶层 `rating`、`color:<name>` 和 `featureData.smartCullingV2`。
6. 保留 adjustments、exif、普通 tags 和其他 featureData 子键。
7. 在同目录写临时文件，flush 后原子替换；失败不得破坏旧 sidecar。
8. 单项成功立即计入成功清单；后续失败不回滚已经成功的其他照片。
9. 写入成功后通过既有 Library 刷新/元数据事件同步 UI。

V2 不写撤销 journal。复核页取消勾选只代表本次不采用，不写 sidecar，也不创建人工保护。

## 13. 前端交互架构

前端状态以 Rust 任务快照为事实源；组件只保存短暂视觉状态。UI 覆盖完整生命周期：

- Library 入口和任务状态入口
- 设备不支持
- 配置和关键人物选择
- 后台进度与取消
- 已完成通知和主动进入复核
- 文件夹树/故事段落/相似组复核
- 多选、全选、修改、完全放弃
- 确认写入、部分成功、失败重试
- Library 星级、颜色、AI/人工来源与原因展示

已验收原型是最终视觉与交互参考，但阶段 P1 的生产 UI 只启用真实后端已支持的能力；不得用假数据或
假按钮伪装后续阶段完成。

## 14. 本地化

智能选图文案由 feature 自己维护中文和英文词典。当前应用语言为中文时使用中文，其余语言缺少对应
词条时回退英文。不得继续向所有主 locale 文件批量加入智能选图文案。

错误由稳定错误码映射为本地文本；文件路径、数量和底层原因作为参数，不把 Rust 英文错误直接展示。

## 15. 性能与资源预算

产品目标为 1000 张在目标低配支持设备上尽量 5 分钟内完成；不把目标放宽为 10 分钟。这个目标必须
用真实 RAW/JPEG 混合、包含调整的目录验证，不能由单张模型 smoke test推导。

每次基准至少记录：

- 设备、OS、GPU/驱动、内存、显存、运行时和模型版本
- RAW/JPEG 比例、分辨率、已编辑比例和目录层级
- 扫描、解码、当前渲染、快速推理、精查、分组和写入耗时
- 峰值 RSS/显存、吞吐、精查比例、失败/跳过数
- 取消响应时间和对 Library 浏览流畅度的影响

性能策略优先减少重复工作：一次扫描、资产级去重、缓存调整摘要、批量推理、候选精查、背压和受控
并发。不得通过跳过当前渲染状态、关闭必需模型或改用低质量结果来“达标”。

## 16. 测试与 Gate

### G0 文档和边界

- 产品、技术、实施、UI 和决策归档语义一致。
- 当前 HEAD 的 V1 清单和所有权基线存在且校验通过。
- feature 目录外变更只能是批准的注册/网关接缝，且逐行可归属本项目二开。

### G1 数据安全闭环

- 目录递归、格式过滤、RAW/JPEG 配对和人工保护单测通过。
- 未确认零写入、取消零覆盖、原子替换、基线冲突和部分成功通过故障注入。
- 普通 Library 修改 AI 结果后变人工、清原因并受保护。

### G2 画面与设备能力

- 当前渲染输入与现有预览在调整/裁剪/蒙版/LUT 样本上一致。
- macOS/Windows 目标设备 GPU provider、模型契约和异常路径通过。
- 不支持设备被明确拒绝，无生产 CPU fallback。

### G3 效果与性能

- 真实摄影验证集覆盖九模式、人物、连拍、歧义配对和受保护组合。
- 召回、组内 3-5 张、多样性、原因真实性和误标红达到冻结门槛。
- 1000 张目标设备基准向 5 分钟目标收敛，资源峰值在预算内。

### G4 完整产品体验

- 已验收 UI 的所有生命周期页面使用真实数据和真实状态。
- 中英文、键盘、缩放、窗口最小尺寸、取消、重试和恢复路径通过。
- 安装包包含通过验证的模型与许可清单，离线环境可完整启动。

任一 Gate 没有当前分支可复现证据时都不得标记通过。

## 17. 分阶段交付

### P0：再冻结与技术刺探

- 建立当前 HEAD 所有权和 V1 清单。
- 验证无 JPEG/IPC 的当前渲染输入端口。
- 验证目标平台 GPU 推理 provider、模型契约、资源和基准方法。
- 冻结 schema、状态机、错误码和性能/效果门槛。

### P1：安全可复核闭环

- 单一网关、状态机、扫描、配对、人工保护和临时结果。
- 使用可替换且明确标注为开发策略的评分端口打通真实流程。
- 复核修改、确认、原子写入、冲突、部分成功和 Library 来源显示。
- 未通过 G1 前不接大模型或故事工作台。

### P2：效果能力

- 接入通过 Gate 的质量、内容、人脸/眼睛和模式路由模型。
- 实现关键人物、受保护组合、相似组 3-5 张和确定性原因。
- 完成目标 macOS/Windows 设备效果与性能验证。

### P3：完整体验

- 文件夹层级、故事段落、复核工作台和全部异常页。
- 中英文与无障碍完善。
- 安装包资源、许可和离线发布验收。

## 18. 禁止事项

- 禁止在未验证前声称 GPU-only、5 分钟、九模式或关键人物已完成。
- 禁止把内部检查点、未提交工作树或历史文档勾选项当成当前 Gate 证据。
- 禁止直接复用 V1 的删除、零星、直接写入和固定阈值 UI。
- 禁止在前端逐张传输完整渲染像素。
- 禁止复制一套调整、裁剪、蒙版或 LUT 算法。
- 禁止覆盖整份 `.rrdata`、人工结果或其他 featureData。
- 禁止加入云端、运行时模型下载、CPU 静默降级、任务历史、撤销、PDF 和偏好学习。
- 禁止为智能选图修改上游组件内部业务逻辑；若现有通用 slot 不足，先提交最小通用契约评审。
