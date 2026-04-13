# QRaw AI 智能选图（Auto Culling）技术方案设计文档

## 1. 项目背景与目标

### 1.1 问题定义
摄影师在单次拍摄任务（如婚礼、活动、人像写真）中往往会产生数以千计的 RAW 照片。其中包含大量连拍冗余、失焦废片、曝光失误或闭眼/表情不佳的照片。
传统的手工筛选（Culling）极其耗时。本项目旨在利用 QRaw 现有的高性能本地计算基建（Rust + ONNX Runtime + WebGPU），实现一个**全自动的 AI 智能选图系统**。

### 1.2 核心目标
系统不再尝试用一个“玄学黑盒模型”去理解美，而是通过**“多维度基础特征提取 + 规则系统融合 + 连拍去重降维”**的工程化手段，为每张照片输出一个量化的**综合评分（1-5 星）**，帮助用户一键过滤废片，选出各场景的最优照片。

---

## 2. 架构设计：多模型组合与规则融合

AI 选图本质上是一个多阶段的流水线（Pipeline）。系统将分为三个主要阶段：特征提取、全局相似度分析、综合打分与评级。

### 2.1 整体数据流向
```text
[RAW 图像目录] 
      ↓ (并行解码与缩略图生成)
[特征提取层 (并行)]
  ├── 清晰度检测 (Laplacian 方差)
  ├── 曝光与色彩健康度 (基于现有的 perform_auto_analysis)
  ├── 语义与美学特征 (基于 CLIP Embedding)
  └── 人脸与表情检测 (待引入轻量级 ONNX 模型)
      ↓ (特征聚合)
[全局分析层 (内存聚合)]
  └── 连拍去重与相似度聚类 (基于 CLIP Embedding 余弦相似度 + 拍摄时间)
      ↓ (规则引擎)
[打分与评级层]
  ├── 废片直接淘汰 (1星)
  ├── 连拍组内优选 (同组最优 4/5 星，其余降级)
  └── 独立好片提权 (3-5星)
      ↓
[持久化层]
  └── 写入 .rrdata (ImageMetadata.rating) -> 前端 UI 更新照片墙星级
```

---

## 3. 核心技术模块设计与落地细节

### 3.1 清晰度检测（Blur Detection）
**目标**：判断照片是否失焦或存在严重动态模糊。
**方案**：无需重型 AI 模型，使用经典 CV 算法。
*   **算法**：拉普拉斯方差（Variance of Laplacian）。
*   **原理**：计算图像灰度图的拉普拉斯算子，其方差代表了图像中高频边缘信息的丰富程度。方差越低，图像越模糊。
*   **QRaw 落地**：
    *   在 [image_processing.rs](file:///Users/qiaopeng/Desktop/owner/QRaw/src-tauri/src/image_processing.rs) 中新增纯 Rust 实现，利用 `rayon` 并行处理缩略图的 `Luma` 通道。
    *   **复杂度**：极低。

### 3.2 图像质量评分（Exposure & Color Health）
**目标**：判断照片是否存在严重的曝光失误（死黑/死白）或色彩异常（严重偏色）。
**方案**：白嫖 QRaw 现有基建。
*   **QRaw 落地**：
    *   直接复用 [image_processing.rs:L2534](file:///Users/qiaopeng/Desktop/owner/QRaw/src-tauri/src/image_processing.rs#L2534) 中的 `perform_auto_analysis` 函数。
    *   该函数已经实现了对图像高光、阴影、直方图分布、饱和度的精确计算。
    *   **规则**：如果该函数返回的 `exposure` 补偿值极大（如 > +3.0 或 < -3.0），或者 `shadows`/`highlights` 需要极端的恢复，则判定原图曝光质量差，进行扣分。

### 3.3 去重与连拍筛选（Duplicate / Burst Grouping）
**目标**：识别出连续拍摄的相似照片，将其归为一组，并在组内进行“内卷”选优，大幅减少相似照片数量。
**方案**：时间窗粗分组 + 深度学习特征向量的相似度计算（避免 O(n²) 性能爆炸）。
*   **QRaw 落地**：
    *   复用 [ai_processing.rs](file:///Users/qiaopeng/Desktop/owner/QRaw/src-tauri/src/ai_processing.rs) 中已集成的 `CLIP` 模型。
    *   在处理每张图片时，提取 CLIP 的 Image Embedding（一个高维浮点数组）并保存在内存中。
    *   **聚类逻辑**：
        1. **时间窗粗分组**：首先读取所有照片的 EXIF 拍摄时间，将时间间隔小于 `X` 秒（例如 5 秒）的照片划分为一个时间窗口（Time Window）。
        2. **组内精准匹配**：只在同一个时间窗口内部，计算两两图片 Embedding 的**余弦相似度 (Cosine Similarity)**。这样将全局的 `O(n²)` 复杂度降维成了多个微小组内的 `O(k²)`，性能大幅提升。
    *   **规则**：如果相似度 `> 0.92`，则确认为同一“连拍子组”。每组最终只保留综合得分最高的一张，其余标记为连拍冗余（降星）。

### 3.4 构图与美学评分（Composition & Aesthetics）
**目标**：评估照片的构图是否合理，是否具有美感。
**方案**：引入轻量级专用美学打分模型（Aesthetic Quality Model）。
*   **QRaw 落地**：
    *   **避坑指南**：虽然 CLIP 的 Zero-shot Prompt 相似度能一定程度反映语义美感，但对同场景下微小构图差异的敏感度极差，无法稳定用于同组优选。
    *   **新方案**：在 HuggingFace 寻找轻量级的 **AVA 美学评分模型 (如 NIMA 的 ONNX 移植版)**，或者训练一个基于 CLIP Embedding 的轻量级回归头（Linear Regressor）。
    *   该模型直接输出一个 1-10 的连续美学质量分，作为辅助加分项。

### 3.5 人脸与表情评分（Facial Expression & Blink）*【需新增】*
**目标**：检测人脸，判断是否闭眼、表情是否自然（核心刚需）。
**方案**：引入级联的轻量级 ONNX 模型（人脸检测 + 眼睛状态分类）。
*   **QRaw 落地**：
    *   **避坑指南**：轻量级人脸检测模型（如 RetinaFace）通常只输出 5 个关键点，这 5 个点只够做人脸对齐，**绝对不够稳定判断闭眼（EAR）**。
    *   **新方案（两阶段）**：
        1. **Stage 1 (检测)**：使用 RetinaFace 或 YOLOv8-Face 框出人脸。
        2. **Stage 2 (分类)**：裁剪出眼部/脸部区域，送入一个极轻量级的分类网络（如 MobileNetV3 训练的 Blink/Open-Eye Classifier，或者专门的 Expression Classifier），直接输出闭眼概率和微笑概率。
    *   **评分逻辑**：
        1. 闭眼概率 > 阈值 -> 严重扣分/淘汰。
        2. 微笑概率 -> 辅助加分。
        3. 人脸占比过小或无脸，则跳过表情打分。

---

## 4. 评分融合与排序规则（Scoring & Fusion）

在完成特征提取后，系统进入规则引擎进行融合。我们定义总分为 `Total_Score` (0-100)，最终映射为 1-5 星。

### 4.1 权重分配示例
```rust
Total_Score = 
    (Blur_Score * 0.35) +         // 清晰度 (权重最大，决定可用性)
    (Exposure_Health * 0.15) +    // 曝光健康度
    (Aesthetic_Score * 0.20) +    // 美学/构图分
    (Face_Expression_Score * 0.30)// 表情分 (如果有人脸，权重极高；无人脸则将权重按比例分配给其他项)
```

### 4.2 评星决策树 (Decision Tree)
1. **废片一票否决**：
   * 如果 `Blur_Score` 极低（严重失焦） -> `Rating = 1`
   * 如果 `Exposure_Health` 极差（死黑/死白不可救） -> `Rating = 1`
   * 如果有人脸且被判定为“完全闭眼” -> `Rating = 1`
2. **连拍去重逻辑**：
   * 将未被否决的图片按 CLIP Embedding 进行相似度聚类。
   * 对于每个连拍组（组内图片数 > 1）：
     * 选出 `Total_Score` 最高的一张，设为“组内最佳”。
     * “组内最佳”根据其绝对分数映射为 `Rating = 4` 或 `5`。
     * 组内其他照片，无论绝对分数多高，强制降级为 `Rating = 2`（作为备选，默认隐藏）。
3. **独立照片评级**：
   * 对于不属于任何连拍组的照片，直接按 `Total_Score` 映射：
     * `Score > 85` -> `Rating = 5`
     * `Score > 70` -> `Rating = 4`
     * `Score > 50` -> `Rating = 3`
     * `Score <= 50` -> `Rating = 2`

---

## 5. 工程实现步骤 (MVP 阶段规划)

为保证项目可控且能快速验证价值，建议分阶段实现：

### Phase 1: 基础设施与基础评分 (MVP)
**目标**：跑通异步处理流，实现清晰度、曝光的打分，并完成基于时间窗的连拍去重。这是验证用户价值的第一步。
1. **扩展数据结构**：在 [image_processing.rs](file:///Users/qiaopeng/Desktop/owner/QRaw/src-tauri/src/image_processing.rs) 的 `ImageMetadata` 旁边定义 `AutoRatingStats` 结构体暂存特征。
2. **实现 Laplacian 算子**：在 `image_processing.rs` 中手写一个基于 `rayon` 的拉普拉斯方差计算函数。
3. **改造 CLIP 推理与时间窗聚类**：提取 CLIP Embedding，结合 EXIF 时间戳，实现 `O(k²)` 的局部去重逻辑。
4. **异步调度流**：在 [tagging.rs](file:///Users/qiaopeng/Desktop/owner/QRaw/src-tauri/src/tagging.rs) 中新增 `start_auto_culling`，并发读取目录、生成缩略图、提取特征、去重并写入基础星级（1-5 星）和可解释标签（如 `{"reason": "连拍重复"}`）。

### Phase 2: 引入人脸与表情模型 (进阶)
**目标**：解决人像摄影的核心痛点（闭眼、表情管理）。
1. **模型选型与级联**：引入 YOLOv8-Face (检测) + 眼睛状态分类器 (闭眼判断) 的 ONNX 模型组合。
2. **ONNX 集成**：在 [ai_processing.rs](file:///Users/qiaopeng/Desktop/owner/QRaw/src-tauri/src/ai_processing.rs) 增加加载逻辑和两阶段推理流水线。
3. **特征加入融合**：将精准的闭眼惩罚和微笑加分加入打分公式中。

### Phase 3: 专用美学打分与评测闭环 (完善)
**目标**：提升构图优选的准确率，并建立科学的调参机制。
1. **美学模型集成**：抛弃 CLIP Prompt 方案，引入专用的 NIMA 等轻量级美学评分 ONNX 模型。
2. **构建评测集**：收集数百张覆盖婚礼、人像、活动的标注数据集。
3. **权重自适应**：基于评测集反馈，调整 `Total_Score` 中各项权重的比例，甚至根据照片是否存在人脸，动态切换打分策略。

---

## 6. 技术栈与性能考量

本方案**完全不使用 Python**，严格遵循 QRaw 的高性能本地化架构。

*   **并行计算**：严重依赖 `rayon`（CPU 密集型如 Laplacian）和 `tokio`（IO 密集型如读取文件和元数据）。
*   **内存管理**：避免将几千张图片的完整像素读入内存。所有评分（除了去重矩阵）均在缩略图（如 1024x1024 甚至 512x512）上即时计算完毕并丢弃图像数据，只保留轻量的 `AutoRatingStats` 结构体。
*   **ONNX 优化**：复用 `ort` 引擎，确保 CLIP 和未来的人脸模型能够利用 CoreML (macOS)、DirectML (Windows) 或 TensorRT 加速。

---
*文档版本：v1.0 (MVP 规划版)*
*基于 QRaw 当前基建状态生成*