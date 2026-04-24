# RapidRAW 分析式风格迁移技术架构 V4（分析式主路径增强版）

---

# 一、文档目的

本文档用于替换 `V3` 中“分析式 + 生成式双轨并存”的叙事方式。

当前项目已经明确移除生成式风格迁移代码路径，因此新的目标不是讨论“双路线如何取舍”，而是回答一个更现实的问题：

```text
在只保留分析式风格迁移的前提下
如何把效果质量继续往上推
并且仍然保持可解释、可编辑、可落地
```

本文档给出：

- 新的产品边界
- 当前分析式实现的主要短板
- 可参考的开源技术方向
- 适合 RapidRAW 的增强架构
- 按当前仓库结构可执行的 Phase 1 / 2 / 3 路线

## 1.1 文档维护原则

从本版本开始，本文档不仅是方案说明文档，也同时承担讨论决策沉淀职责。

必须明确：

- 每一轮讨论形成的共识都需要回写文档
- 已确认事项应写入正式章节，成为后续实现约束
- 尚未确认但已形成明确方向的事项，应写入“讨论决策记录”
- 待验证问题必须显式标记为“需要 benchmark / 调研 / 实验验证”

这样做的目的不是增加文档负担，而是避免后续实现时丢失上下文、重复讨论或误解边界。

---

# 二、当前结论

## 2.1 已确认事实

- 生成式风格迁移在真实大图场景下等待时间超过 30 分钟
- 该等待成本不符合专业摄影工作流
- 生成式路径已经从当前项目中移除
- 当前项目实际只保留分析式风格迁移能力

因此，从产品视角看：

```text
RapidRAW 的风格迁移主路径
只能是分析式
```

## 2.2 当前困境

当前分析式路径虽然满足了速度和可控性要求，但效果质量仍不理想。

这种“不理想”主要不是指它完全无效，而是指它目前更像：

- 全局色调接近
- 全局参数建议
- 基础 HSL / Curve / LUT 对齐

但还不像：

- 摄影师真正会认同的风格迁移
- 对肤色、天空、植被、背景有区别对待的风格组织
- 能稳定迁移“look”而不是只迁移“平均颜色”

---

# 三、产品边界重新定义

## 3.1 分析式风格迁移的定义

分析式风格迁移仍然定义为：

- 输入参考图和当前图
- 输出可编辑的调色建议
- 不生成新结构
- 不重绘内容
- 不改变 RAW 非破坏性工作流语义

其输出必须属于下列对象之一：

- 全局调整参数
- 曲线
- HSL 分区参数
- LUT
- 局部区域参数建议
- 与现有编辑系统兼容的 mask-based 参数块

## 3.2 不再承诺的能力

分析式路线不应承诺以下能力：

- 重写光照方向
- 重造天气氛围
- 重绘复杂材质
- 改写主体结构
- 生成不存在的视觉元素

这些能力本质上更接近生成式重绘，不应再作为分析式产品目标。

## 3.3 必须承诺的能力

分析式路线必须重点提升以下能力：

- 影调结构接近度
- 色彩语言接近度
- 肤色稳定性
- 高光安全性
- 天空 / 植被 / 背景等区域的风格分离控制
- 输出结果的可解释性与可编辑性

## 3.4 新增硬约束

从本版本开始，以下三条不再属于建议，而属于必须满足的硬约束。

### A. 时间上限硬约束

```text
2 分钟是默认等待上限
超过 2 分钟不得静默继续
必须提示风险并由用户明确选择是否继续等待
```

补充说明：

- 常规目标仍然应保持在 1 到 5 秒
- 超过 10 秒应视为异常并记录诊断信息
- 2 分钟是大多数用户可容忍的默认耐心边界
- 一切算法设计都必须优先服从该时间上限
- 质量仍然高于低配机器上的无条件快返回
- 对低配机器，不应为了强行卡进 2 分钟而默认做不可逆质量降级

### B. 原图质量与分辨率硬约束

```text
任何分析与映射过程
都不得修改用户原图的分辨率、像素数量和基础画质
```

必须明确：

- 低分辨率图仅允许作为分析输入或中间特征载体
- 最终应用仍然作用于原始分辨率图像
- 不允许把低分辨率分析结果直接导出为最终结果
- 不允许为了提速而默认降采样用户原图后再写回

### C. 结果语义硬约束

```text
分析结果必须延续现有模式
必须能映射到当前调试滑块和参数系统
并且允许用户继续微调
```

因此：

- 风格迁移输出不能成为黑箱图片结果
- 结构化结果必须能落到现有 adjustment / curve / HSL / LUT / mask 语义
- 关键分析结论必须能映射到 UI 中可见的调试项
- 用户必须可以继续手动修正系统建议

## 3.6 运行模式与用户选择边界

本轮讨论已经确认，分析式风格迁移不应只有一种默认强度，而应允许用户在开始前明确选择策略模式。

### A. 双模式策略

V4 至少需要支持两种策略模式：

- `安全起点模式`
- `强风格迁移模式`

它们不应只是一个简单强度滑杆，而应代表两套不同的运行策略。

### B. 安全起点模式

该模式应优先保证：

- 肤色安全
- 高光安全
- 参数保守
- 适合作为后续手工调色起点

### C. 强风格迁移模式

该模式应优先保证：

- 更强的参考图接近度
- 更激进的 HSL / 曲线 / LUT / 局部映射
- 即使跨题材也继续尝试强迁移

但必须在风险较高时明确提示用户。

### D. 风险处理原则

对于“跨题材差异大、结果存在不稳定风险”的情况：

- 默认仍按强迁移模式继续运行
- 但必须在 UI 中提示风险
- 是否继续由用户决定

## 3.5 场景兼容硬约束

V4 必须明确支持以下真实使用场景：

```text
用户拿一张常规图片格式的参考图
去分析其风格
并把结果应用到一张原厂 RAW 上
```

这条要求必须被理解为“跨格式风格迁移”，而不是“输入输出都必须是同一格式”。

### A. 参考图支持范围

作为参考图参与风格分析的输入，至少必须支持：

- `jpg`
- `jpeg`
- `png`
- `tiff`
- `tif`

补充说明：

- 参考图不要求必须是 RAW
- 参考图可以来自手机、相机导出图、网络样张或摄影成片
- 参考图的职责是提供风格语言，而不是提供传感器数据

### B. 被应用目标图支持范围

被应用风格建议的目标图，必须优先支持各大相机厂商原厂 RAW。

至少包括：

- `DNG`
- `NEF`
- `CR2`
- `CR3`
- `ARW`
- `RAF`
- `ORF`
- `RW2`

后续可继续扩展，但分析式风格迁移层不需要自行定义额外 RAW 兼容矩阵。

兼容原则应为：

- 基建支持什么 RAW 格式，风格迁移层就继承支持什么
- 二次开发层不单独重新裁剪 RAW 支持范围
- 若某类 RAW 在基建中支持不稳定，应按基建实际能力对外表述

### C. 跨格式应用原则

必须明确：

- 参考图可以是常规格式
- 当前图可以是原厂 RAW
- 系统必须支持“常规格式参考图 -> RAW 调色建议”的完整链路

不允许因为参考图不是 RAW，就把风格迁移能力降级为不可用。

### D. 格式语义边界

不同格式在系统中的职责必须区分清楚：

- 参考图负责提供风格信息
- RAW 当前图负责承接可编辑参数建议
- 最终被修改的是 RAW 编辑状态，而不是参考图文件本身
- 参考图格式不应限制目标 RAW 的编辑质量与分辨率

---

# 四、对当前实现的判断

## 4.1 当前实现的优点

基于当前仓库代码，现有分析式并不属于“只有几条简单规则”的弱实现，而是已经具备以下能力雏形：

- 图像风格特征提取
- 风格差异评分
- 专家预设匹配
- 参数映射
- 曲线生成
- HSL 映射
- 自动二次微调
- 动态约束与质量护栏
- OT + TPS 生成 3D LUT

这说明当前系统不是从零开始，而是已经有一个可继续演进的底座。

## 4.2 当前实现的主要短板

当前效果上限主要受以下问题限制。

### A. 输入域不统一

当前分析优先读取标准图像，RAW 则优先提取 preview，而不是统一落到“可控的 RAW 规范分析域”。

这会带来问题：

- 不同相机预览图风格差异很大
- 相机厂商预览曲线会污染分析结果
- 同一张 RAW 在不同预览来源下特征不稳定

### B. 当前特征更偏全局统计

现有特征已经覆盖亮度、饱和度、肤色、高光、色相等维度，但主体仍是全局统计和弱空间统计。

这会导致：

- 对天空和地面采用同一类映射
- 对人物肤色和背景缺少真正的分区控制
- 风格迁移更像“平均 look 对齐”，而不像“区域组织对齐”

### C. 局部建模能力不够

当前系统能输出曲线、HSL、LUT，但缺少足够强的局部仿射映射或语义区域映射。

因此会出现：

- 某些颜色区域对齐后，另一些区域被连带拉坏
- 肤色与环境色冲突
- 高光保护和色彩迁移相互牵制

### D. 专家预设仍以手工规则为主

当前 `expert_presets` 仍是手工预设集合。

它可以作为经验底座，但很难覆盖：

- 真实摄影风格的连续空间
- 场景到参数的复杂映射
- 不同机型 / 不同曝光状态 / 不同题材下的非线性差异

### E. 质量评价仍偏“内部指标”

当前系统已经有 style debug，但还缺少更完整的产品评测闭环，例如：

- 风格接近度的主观打分标准
- 分区质量标准
- 人像专项评测
- 数据集回归机制

---

# 五、V4 的核心判断

V4 的核心判断如下：

```text
不要继续堆更多全局规则
而要把分析式升级为
“可学习的确定性色彩映射 + 语义分区 + 可编辑参数回写”
```

换句话说，V4 不是回到生成式，也不是只做更复杂的 heuristic，而是进入下面这条路线：

- 主路径仍然是分析式
- 运行时仍然是快速、确定性、可解释
- 质量提升主要来自更好的输入域、更强的局部映射和数据驱动的 preset 学习

---

# 六、适合 RapidRAW 的技术路线

## 6.1 总体方向

V4 推荐采用三层式分析架构：

```text
规范输入层
    ↓
风格理解层
    ↓
可编辑映射层
```

对应目标分别是：

- 规范输入层：把不同来源图像变成稳定可分析对象
- 风格理解层：理解全局风格 + 局部区域风格
- 可编辑映射层：把风格理解结果回写成可编辑参数，而不是只输出一张结果图

## 6.2 规范输入层

### 目标

- 统一 JPEG / TIFF / RAW 的分析入口
- 降低相机预览风格对结果的污染
- 让不同文件来源下的特征可比较
- 支持“常规格式参考图 + 原厂 RAW 当前图”的跨格式分析链路

### 推荐做法

- RAW 输入先生成一张低分辨率 `canonical RAW preview`
- 常规格式参考图也应落到统一分析色彩空间
- 该 preview 应尽量基于统一的 demosaic、白平衡和色彩空间规范化
- 分析特征尽量在高 bit-depth 线性或近线性空间上提取
- 非 RAW 文件也尽量落到统一分析色彩空间

这里的低分辨率 `canonical RAW preview` 必须理解为：

- 它只服务于分析
- 它不替代原图
- 它不改变最终应用分辨率
- 它只负责生成稳定、统一、低成本的分析特征

也就是说：

```text
低分辨率发生在“理解图片”阶段
不发生在“修改用户原图”阶段
```

这一策略是否替换当前 embedded preview 直读路径，必须通过基准验证后决定。

验证至少应回答：

- 首次分析比当前方案慢多少
- 是否仍能落在用户可接受的默认等待区间内
- 对结果可靠性提升多少
- 对参考图接近度提升多少

只有当结论整体为正向收益时，才应正式切换为统一 `canonical RAW preview` 主路径。

### 不推荐做法

- 直接依赖相机 embedded preview 作为唯一分析输入
- JPEG 走一套分析口径、RAW 走另一套口径
- 让相机品牌默认色彩直接主导风格判断
- 因为参考图是 `jpg/png/tiff` 就放弃对 RAW 当前图的风格应用

## 6.3 风格理解层

风格理解层拆成三部分。

### A. 全局风格编码

继续保留当前已有优势：

- 亮度分布
- 对比度分布
- 饱和度分布
- 色相统计
- 肤色统计
- 高光 / 阴影风险
- 暗角 / 纹理 / 清晰度信号

### B. 语义区域编码

新增最关键的能力：

- skin
- sky
- vegetation
- background / architecture
- subject / non-subject

语义区域不需要一开始就做非常重的分割网络。

V4 的务实建议是：

- Phase 1 先做人像肤色区域和天空区域
- Phase 2 扩展到植被与背景
- Phase 3 再做更完整的语义区域层

### C. 风格原型与 preset 嵌入

不要只保留手工 preset 枚举，而要引入：

- 风格原型向量
- preset embedding
- scene-to-preset ranker

也就是说，系统需要学会回答：

```text
这张参考图更接近哪一类风格原型
当前图离这个原型还差哪些局部和全局特征
```

## 6.4 可编辑映射层

V4 的可编辑映射层建议同时输出三类结果。

### A. 全局参数头

输出当前编辑器已有参数：

- exposure
- contrast
- highlights
- shadows
- whites
- blacks
- saturation
- vibrance
- temperature
- tint
- clarity
- dehaze
- structure
- sharpness

### B. 曲线 / HSL / LUT 头

继续保留并增强：

- 曲线
- HSL 八分区
- 3D LUT

但要从“单一全局 LUT”升级到“受控的低秩 LUT 或 basis LUT 混合”。

### C. 局部参数头

这是 V4 效果提升的关键。

局部参数头输出的是：

- 区域 mask
- 区域参数块
- 区域 LUT 或区域 tone bias

在产品语义上，它仍然属于编辑建议，而不是生成结果。

## 6.5 性能优先的运行策略

为了满足“2 分钟绝对上限”和“常规秒级返回”的目标，V4 必须明确采用极限性能优化策略。

### A. 算法复杂度约束

- 特征提取优先采用单次扫描或少次扫描的线性复杂度流程
- 避免在运行时对整张大图做高复杂度 dense correspondence
- 避免在运行时引入需要大量迭代收敛的重优化求解
- 任何局部映射算法都必须有固定上界

### B. 低分辨率分析、高分辨率应用

- 在分析阶段统一使用低分辨率规范输入
- 在应用阶段把结果映射回原始分辨率图像
- 高分辨率阶段优先执行参数应用、曲线、LUT、局部 mask 混合
- 不允许在最终应用阶段重新做重型全图风格理解

### C. 空间换时间

允许大量使用受控缓存，但必须绑定输入签名。

可缓存对象包括：

- 规范化分析预览
- 全局特征向量
- 区域 mask
- 区域统计量
- 风格原型匹配结果
- basis LUT 权重
- 最近一次分析结果

要求：

- 必须支持失效
- 必须可清理
- 不得用旧缓存伪造新结果

### D. 预计算与常驻

- 常用 LUT basis 应预计算
- 常用风格原型向量应常驻内存
- 常用区域识别模型应常驻
- 高成本映射参数应尽量在离线训练时固化，而不是运行时重新求解

### E. 并行化与底层优化

- 图像统计优先采用 SIMD / Rayon 等并行策略
- 允许将互不依赖的特征提取并行执行
- 大图最终应用阶段应优先使用 scanline / tile 流式处理，而不是整图重复拷贝
- 必须尽量减少中间图像对象的重复分配

### F. 超时策略

- 超过 10 秒：记录慢任务诊断信息
- 超过 30 秒：UI 明确提示任务异常偏慢
- 超过 2 分钟：不得静默继续，必须弹出继续等待确认
- 若用户拒绝继续等待：停止任务并返回可诊断错误
- 若用户确认继续等待：允许任务继续执行，但需要持续显示风险状态

## 6.6 交互与结构化策略补充

### A. 蒙版可见性策略

自动生成的风格迁移局部蒙版，默认应为隐藏状态。

但必须支持：

- 用户主动开启蒙版回显
- 用户查看局部映射范围
- 用户基于回显蒙版进行二次修改

### B. 多参考图语义

V4 支持多参考图时，应采用：

- 一个且仅一个主参考图
- 零个或多个辅助参考图

系统职责如下：

- 主参考图负责定义主要风格方向
- 辅助参考图负责补充局部风格或色彩倾向
- 不应采用所有参考图等权平均的默认策略

### C. 首版能力边界

首版分析式风格迁移只纳入以下能力：

- 色彩
- 影调
- HSL
- 曲线
- 局部色彩偏移

以下效果明确列入后续版本，而不进入首版主路径：

- grain
- halation
- glow
- vignette

---

# 七、V4 推荐的模型与算法路线

## 7.1 最值得参考的开源方向

### A. HDRNet / Bilateral Grid / Local Affine

适合解决的问题：

- 快速
- 可局部化
- 可解释
- 对高分辨率友好

适合 RapidRAW 的原因：

- 它本质上是局部仿射映射，不是生成式
- 可以把低分辨率理解结果应用到高分辨率图像
- 非常适合作为“分析式增强层”

### B. Image-Adaptive 3D LUT / LUT with Bilateral Grid

适合解决的问题：

- 提高色彩风格迁移的非线性表达能力
- 保持运行时足够快
- 易于落成可编辑或半可编辑对象

适合 RapidRAW 的原因：

- 比单个全局 LUT 更灵活
- 比扩散模型更可控
- 更符合本地摄影软件的运行时约束

### C. Neural Preset / Deep Preset

适合解决的问题：

- 从参考图或成对数据中学习“参数空间映射”
- 把输出对齐到编辑器 preset / adjustment 语义

适合 RapidRAW 的原因：

- 与非破坏性工作流高度一致
- 目标不是生成图，而是生成调色操作
- 更容易与现有 sidecar / preset 系统整合

### D. 语义颜色迁移

适合解决的问题：

- 让人物、天空、植被、背景区别对待
- 避免全局颜色迁移破坏局部合理性

适合 RapidRAW 的原因：

- 摄影风格迁移最怕“全局正确、局部翻车”
- 加语义区域通常比继续堆全局规则更有效

## 7.2 不推荐直接照搬的方向

- 纯扩散式风格迁移
- 完全依赖 dense correspondence 的重模型在线推理
- 只输出结果图、不输出可编辑参数的方案
- 只适合 8-bit sRGB 图像社交风格化的路线

## 7.3 本地模型路线调研结论

本轮调研的目标不是单纯比较论文分数，而是比较以下四个维度：

- 是否真正适合“参考图 -> 当前图”的分析式风格迁移
- 是否能在高分辨率照片上稳定落地
- 是否能保留可编辑输出语义
- 是否适合当前 RapidRAW 的本地运行时与部署形态

### 7.3.1 候选路线评估

#### A. Neural Preset

定位：

- 当前最接近“参考图驱动的分析式风格迁移”的公开方向
- 重点是 faithful color style transfer，而不是生成新内容

已确认事实：

- 官方论文明确面向 color style transfer
- 论文表格给出的模型规模为 `5.15M` 参数
- 论文表格给出的 GPU 推理时间约为 `0.019s @ 4K`
- 补充材料给出的 CPU 推理时间约为 `0.686s @ 4K`
- 官方仓库明确强调其目标是 `Faithful 4K Color Style Transfer in Real Time`

适合 RapidRAW 的原因：

- 它比 LUT-only 路线更接近“参考图驱动”
- 它本质上仍是确定性色彩映射，不是扩散生成
- 它在高分辨率推理上的公开结果是积极的

主要风险：

- 官方实现是研究代码，不是面向 Rust / Tauri / ONNX Runtime 的现成生产接入
- 其输出默认更接近“直接得到 stylized result”，不是直接输出你现有的滑块语义
- 若直接照搬，需要解决可编辑参数回写问题

部署判断：

- 模型体量属于“可本地化，但不算极小”的级别
- 以 `5.15M` 参数估算，纯权重体积大致处于“十几 MB 到几十 MB”区间，取决于精度与导出格式
- 可作为“风格参数预测头”的主要灵感来源
- 不建议直接把官方研究实现原样塞进运行时主链路

#### B. Image-Adaptive 3D LUT

定位：

- 适合作为超快的本地增强底座
- 重点在 photo enhancement，不是参考图驱动风格迁移

已确认事实：

- 官方仓库说明模型参数少于 `600K`
- 官方仓库说明 `4K` 图像推理时间少于 `2ms`
- 官方仓库明确说明可在 `480p` 训练后应用到 `4K` 或更高分辨率而不影响表现

适合 RapidRAW 的原因：

- 体积小
- 高分辨率友好
- 非常适合“低分辨率分析，高分辨率应用”
- 与 GPU LUT 应用链天然相容

主要风险：

- 更偏 enhancement baseline，而不是 reference-conditioned style transfer
- 单纯全局 3D LUT 表达能力有限
- 对局部语义差异的处理能力不足

部署判断：

- 本地部署代价很低
- 以 `<600K` 参数估算，纯权重通常只在数 MB 量级
- 非常适合作为 V4 的高分辨率执行底座
- 不适合作为唯一的“风格理解核心”

#### C. LUTwithBGrid / Spatial-aware LUT 系列

定位：

- 在 3D LUT 基础上引入空间感知
- 试图解决“全局 LUT 无法区别天空、人物、背景”的问题

已确认事实：

- 官方 ECCV 2024 仓库使用 bilateral grid + LUT
- 官方仓库需要编译 LUT transform 与 bilateral slicing 扩展
- 官方补充材料给出 `4K` 运行时间约为 `3.64ms`
- 该路线明显比纯全局 3D LUT 更重，但仍处于实时级

适合 RapidRAW 的原因：

- 比全局 LUT 更适合处理局部差异
- 更接近你对人像、天空、植被、室内区域差异化迁移的需求

主要风险：

- 官方实现依赖 PyTorch + 编译扩展，生产接入成本明显高于 ONNX 普通模型
- 它仍然主要是 enhancement 路线，不是强参考图条件建模
- 若不重写运行时，直接集成维护成本偏高

部署判断：

- 作为“结构参考”很有价值
- 作为“原样引入的运行时依赖”并不理想
- 更适合把思想蒸馏到你自己的局部 LUT / local affine 模块，而不是直接搬仓库

#### D. SVDLUT

定位：

- 当前已公开开源路线里，速度 / 模型体积 / 高分辨率表现最强的一档 LUT 系列
- 本质仍是 image enhancement 路线，不是参考图驱动主模型

已确认事实：

- ICCV 2025 论文表格给出的模型规模为 `160.5K` 参数
- 论文表格给出的 `4K` 推理时间约为 `1.38ms`
- 论文中对比图明确把它放在性能 / 运行时间 / 模型尺寸 trade-off 的最优一档

适合 RapidRAW 的原因：

- 极度适合作为本地高分辨率应用器
- 模型小，冷启动和安装成本低
- 对你的“2 分钟内体验底线”几乎没有压力

主要风险：

- 不是参考图条件化风格迁移方案
- 更适合作为“执行器”而不是“风格理解器”

部署判断：

- 如果只看本地部署难度，它是当前最优底座之一
- 以 `160.5K` 参数估算，纯权重体积可压到约 `1MB` 左右量级
- 适合做 Phase 1/2 的高性能映射头或高分辨率执行头

#### E. HDRNet

定位：

- 经典的 bilateral grid / local affine 路线
- 主要价值在于架构思想，而不是直接作为你项目的最终实现方案

已确认事实：

- 官方项目面向 real-time image enhancement
- 官方仓库已经归档
- 官方仓库明确说明部分预训练模型要求 `16-bit linear input`
- 官方仓库明确警告：如果直接对 `sRGB` 输入使用对应模型，结果可能出现不自然颜色

适合 RapidRAW 的原因：

- 它进一步证明“规范输入 + 局部仿射 + 高分辨率应用”是正确方向
- 对你推动 `canonical RAW preview` 的必要性有直接支撑

主要风险：

- 工程栈较老
- 官方实现依赖自定义 TensorFlow operator
- 并非面向当代桌面 Rust / ONNX Runtime 集成

部署判断：

- 更适合作为架构参考
- 不建议直接作为 V4 的最终运行时实现目标

#### F. SA-LUT

定位：

- 截至当前调研阶段，公开可见的新一代 photorealistic style transfer 强候选
- 相比传统 3D LUT，更强调空间自适应和 reference-conditioned 风格迁移

已确认事实：

- ICCV 2025 论文明确提出 `Spatial Adaptive 4D Look-Up Table`
- 论文声称相对传统 3D LUT 路线显著降低 LPIPS，并维持视频 `16 FPS` 级别实时性能
- 官方仓库已公开
- 官方仓库 README 明确写明 checkpoint 约 `208 MB`
- 官方仓库 README 明确写明其当前实现依赖 Linux、Conda、CUDA 与自定义扩展构建

适合 RapidRAW 的原因：

- 从任务定义上，它比 `Neural Preset` 更接近“新一代 photorealistic style transfer”
- 它兼顾参考图驱动与局部空间适应
- 理论上更接近你想要的“跨题材、局部不翻车”的目标

主要风险：

- 官方实现当前不是跨平台桌面产品级集成形态
- 依赖自定义 CUDA 扩展，不适合直接塞进现有 `Rust + ONNX Runtime + wgpu` 主链
- 输出仍然偏向结果图生成，而不是天然输出你现有的滑块与局部参数语义

部署判断：

- 如果只看“模型包体积”，它完全在可接受范围内
- 如果看“工程接入风险”，它明显高于 `Neural Preset` 和小型 LUT 执行头
- 更适合作为 teacher、蒸馏目标、对照基线或研究参考
- 不适合作为 V4 Phase 1 的直接运行时主模型

#### G. 大视觉编码器路线（DINOv2 等）

定位：

- 不是直接做风格迁移的最终模型
- 而是作为更强的 style understanding backbone

已确认事实：

- DINOv2 官方仓库提供多档 backbone
- 官方给出的参数规模包括 `21M / 86M / 300M / 1100M`

适合 RapidRAW 的原因：

- 2GB 预算允许我们考虑更强的冻结视觉 backbone
- 这类 backbone 更适合提升跨题材、跨构图时的参考图理解能力
- 比盲目增大 LUT 执行头更可能带来“理解能力”的提升

主要风险：

- 这类 backbone 不是针对摄影风格迁移专门训练的
- 即便体积允许，也不代表跨平台推理代价和冷启动代价合理
- 若没有蒸馏或轻量头配合，容易把系统做成“能装下，但不值得”

部署判断：

- `ViT-B` 级别是现实候选
- `ViT-L` 级别只有在 benchmark 明确证明收益时才值得考虑
- `ViT-g` 级别虽然可能仍在 2GB 总预算内，但对桌面跨平台产品并不划算

### 7.3.2 综合结论

如果问题是“当前公开开源里，最像你要做的分析式风格迁移是哪条线”，答案是：

- `Neural Preset`

如果问题是“当前最适合本地高性能部署的执行底座是哪条线”，答案是：

- `SVDLUT / 3D LUT / spatial-aware LUT` 这一族

如果问题是“在 2GB 模型预算下，有没有比前面更值得重视的新候选”，答案是：

- 有，`SA-LUT` 值得纳入研究清单
- 但它改变的是“研究与蒸馏优先级”，而不是运行时主链的工程原则

因此，V4 不应在两者之间二选一，而应采用混合路线：

- 用参考图条件模型负责“理解风格”
- 用 LUT / local affine / 局部参数模块负责“高速应用风格”
- 最终输出仍落到可编辑滑块、曲线、HSL、LUT、mask 参数块

### 7.3.3 对 RapidRAW 的正式建议

V4 正式建议如下：

1. 不直接把论文官方 PyTorch 研究代码作为运行时主链路接入。
2. 运行时主链路优先采用 `ONNX Runtime + 小模型 + GPU LUT / shader 应用`。
3. 参考图风格理解部分优先借鉴 `Neural Preset`，但输出必须改造成：
   - 全局参数建议
   - 曲线
   - HSL
   - 全局 LUT / 局部 LUT 权重
   - 局部区域参数块
   - `sliderMapping`
4. 高分辨率应用部分优先借鉴 `SVDLUT / Image-Adaptive 3D LUT / spatial-aware LUT`。
5. `HDRNet` 主要作为“为什么必须使用 canonical RAW input”的理论与工程佐证，不作为直接实现目标。
6. 在 `2GB` 模型预算前提下，可新增两条增强策略：
   - 使用更强的冻结视觉 backbone 提升 style understanding
   - 将 `SA-LUT` 作为 teacher 或蒸馏参考，而不是直接产品化接入

### 7.3.4 当前阶段的推荐选型

若按“效果、性能、可编辑性、工程代价”四项综合排序，当前建议如下：

- 风格理解主路线：`Neural Preset` 思想蒸馏版
- 高分辨率执行主路线：`SVDLUT` 风格的小型 LUT 执行头
- 局部区域增强参考：`LUTwithBGrid` / spatial-aware LUT
- 架构思想参考：`HDRNet`

在 `2GB` 模型预算下的修正版建议：

- 运行时主路线仍不变
- 但允许把 style understanding backbone 从“小模型”升级到“中型冻结编码器 + 小预测头”
- 同时把 `SA-LUT` 纳入 benchmark 与蒸馏教师候选

这意味着：

- 2GB 预算会改变“模型配置上限”
- 但不会改变“结构化输出 + 高分辨率执行器 + 可编辑回写”这条总路线
- 更大的模型包不应该被用来引入重生成式主模型，而应该优先投入到更强的风格理解能力

也就是说：

- 最强的“风格迁移思想来源”不是 SVDLUT，而是 Neural Preset
- 最强的“本地执行底座”不是 Neural Preset，而是 SVDLUT
- 真正适合 RapidRAW 的是两者融合，而不是单模型崇拜

### 7.3.5 当前拍板建议

在本轮讨论后，V4 对这两个未决技术点给出正式判断：

#### A. Phase 1 是否坚持纯跨平台 ONNX 主链

正式结论：

- 是，Phase 1 必须坚持 `纯跨平台 ONNX Runtime 主链`
- 不为 `CUDA-only`、`Linux-only`、自定义扩展强依赖的研究模型开运行时例外

原因：

- 当前项目本身就是桌面跨平台产品，不是单平台研究原型
- 仓库已经具备 `ort + wgpu` 的可复用基建，继续沿主链推进风险最低
- 一旦为某个 `CUDA-only` 模型开运行时例外，后续会立刻带来：
  - 平台能力分叉
  - QA 成本放大
  - 用户体验不一致
  - 维护负担长期固化
- `SA-LUT` 这类研究实现仍然有价值，但应放在：
  - benchmark
  - teacher
  - 蒸馏参考
  - 离线实验

边界说明：

- 离线训练与研究工具不受此限制
- 但正式产品运行时主链必须保持统一

#### B. 风格理解 backbone 首版选择

正式结论：

- 首版默认采用 `ViT-B` 级别的中型冻结视觉 backbone
- 不首发同时维护 `ViT-B / ViT-L` 双档运行时方案
- 架构层保留升级到 `ViT-L` 的接口，但不在首版直接落双模型运营

原因：

- `ViT-B` 已经足够明显高于“小卷积头 + 手工特征”的理解上限
- 它能吃下更复杂的跨题材、跨构图、多参考图风格理解任务
- 它的模型体量、冷启动、内存占用、ONNX 导出与跨平台推理代价仍处于可控区间
- `ViT-L` 虽然更强，但首版缺少 benchmark 证据证明其收益足以覆盖：
  - 更慢初始化
  - 更高内存
  - 更复杂的部署与调优

因此，首版推荐配置应为：

- `ViT-B` 冻结 backbone
- 轻量 style head
- 小型结构化参数预测头
- LUT / local affine 高分辨率执行器

后续升级条件：

- 若 benchmark 证明 `ViT-L` 在跨题材一致性、局部不翻车率、多参考图融合上有显著优势
- 且不破坏大多数机器的等待体验
- 再评估是否引入 `ViT-L` 版本

---

# 八、V4 系统架构

## 8.1 架构核心思想

```text
Rust / Tauri = 主运行时与编辑语义中心
可选 Python = 离线训练与评测工具
```

必须明确：

- 运行时主链路不依赖生成式服务
- 分析式风格迁移应优先在本地快速完成
- Python 可以存在，但不再承担主产品交互链路
- Python 更适合作为训练脚本、评测脚本和数据处理工具
- 最终结果必须以原始分辨率工作流为准，不允许降分辨率写回用户资产
- 分析结果必须可写回 sidecar，保证结果可复现、可继续编辑

## 8.2 推荐模块拆分

### Rust 运行时模块

- `style-transfer-runtime`
  - 负责任务编排
  - 负责调用分析链路
  - 负责把输出结果包装成当前前端可消费格式

- `style-transfer-canonical-input`
  - 负责 RAW / JPEG / TIFF 统一分析入口
  - 负责生成规范化分析预览

- `style-transfer-feature-extractor`
  - 负责全局特征提取
  - 负责区域级特征提取

- `style-transfer-semantic-segmentation`
  - 负责 skin / sky / vegetation / subject 等轻量区域识别

- `style-transfer-preset-ranker`
  - 负责风格原型匹配
  - 负责专家预设排序
  - 后续可升级为学习型 ranker

- `style-transfer-global-mapper`
  - 负责全局参数建议
  - 负责 curves / HSL / 全局 LUT

- `style-transfer-local-mapper`
  - 负责区域 mask 参数块
  - 负责局部 affine / 局部 LUT

- `style-transfer-quality-guard`
  - 负责肤色保护
  - 负责高光安全
  - 负责局部结果冲突检查

- `style-transfer-evaluator`
  - 负责内部评测得分
  - 负责调试指标输出

### 可选离线训练模块

- `python/style_transfer_training`
  - 数据集清洗
  - 风格对构建
  - preset 学习
  - LUT 基模型训练
  - 回归评测

必须与运行时解耦。

## 8.3 与当前仓库的映射关系

当前建议不是推翻重写，而是在现有代码上逐步拆模块。

### 当前可复用部分

- `src-tauri/src/style_transfer.rs`
  - 可保留为核心逻辑来源
  - 但应从单文件逐步拆模块

- `src-tauri/src/style_transfer_runtime.rs`
  - 可继续作为任务入口与封装层

- `src-tauri/src/expert_presets.rs`
  - 可保留为第一阶段的风格原型底座

- `src/components/panel/right/chat/styleTransfer/*`
  - 可继续复用当前前端承接能力

### 当前应逐步弱化或转用途的部分

- `python/style_transfer_service/*`
  - 不再作为主运行时风格迁移服务
  - 可转为离线训练、评测、实验脚本区域

## 8.4 与当前仓库 AI 基建的接入结论

本轮调研后，已可以正式确认以下事实：

- 当前仓库已经引入 `ort`
- 当前仓库已经存在 ONNX 模型下载、缓存、SHA 校验、初始化与复用逻辑
- 当前仓库已经存在 `wgpu` 与 shader 高分辨率处理链
- 当前仓库已经存在局部蒙版、局部参数与 sidecar 写回能力

这意味着：

- V4 本地模型不需要重新发明运行时模型框架
- 最低风险方案不是继续引入一个新的 Python 服务，而是沿用当前 `ONNX Runtime + Rust + GPU` 主链
- 高分辨率应用应继续放在现有 GPU / shader / LUT 管线
- 小模型负责预测结构化编辑建议，大图执行仍由现有编辑引擎承担

因此，V4 对模型接入方式的正式约束为：

- 优先支持 `ONNX` 导出
- 优先使用当前 `ort` 基建加载
- 不把 PyTorch 自定义 CUDA 扩展作为正式运行时硬依赖
- 不让高分辨率照片经过深层卷积主干逐像素重跑
- Phase 1 正式坚持纯跨平台 `ONNX Runtime` 主链

## 8.5 模型分发与体积约束

本轮讨论已确认以下产品约束：

- 风格迁移所需本地模型应随安装包内置
- 不默认采用首次运行后再下载的方案
- 若模型总包体积在 `2GB` 以内，可接受内置分发

这意味着：

- “模型能否内置”已不再是主限制条件
- 真正限制因素变为：
  - 跨平台可运行性
  - 冷启动与初始化时延
  - 维护复杂度
  - 是否能输出结构化、可编辑结果

因此，V4 对模型体积的正式态度为：

- `2GB` 是允许上限，不是鼓励目标
- 在效果接近时，仍优先选择更小、更稳、更易跨平台部署的方案
- 更大的体积预算应优先用于：
  - 更强的风格理解 backbone
  - 更稳的语义区域模型
  - 更好的多参考图融合能力
- 不应用于引入一条新的重生成式运行时主链

首版默认推荐：

- 内置 `ViT-B` 级别 style understanding backbone
- 不首发维护 `ViT-L` 双档运行时分支
- 将更大 backbone 作为 benchmark 通过后的升级选项

## 8.6 端到端运行流程（从聊天上传到最终应用）

本节给出 V4 在产品中的标准主流程。

目标是回答一个具体问题：

- 用户在聊天窗口上传主参考图和辅助参考图后
- 系统到底如何一步步把“参考图风格”
- 变成“可编辑的滑块、曲线、HSL、LUT、局部蒙版参数”
- 并最终应用到目标图像上

### 8.6.1 输入对象

用户发起一次风格迁移时，输入至少包括：

- `targetImage`
  - 当前编辑目标图
  - 可以是 RAW，也可以是常规格式
- `mainReference`
  - 主参考图
  - 必传且唯一
- `auxReferences`
  - 辅助参考图列表
  - 可为空，也可多张
- `mode`
  - `safe`
  - `strong`
- `userOptions`
  - 强度
  - 是否启用局部建议
  - 是否允许多参考融合
  - 是否允许自动写回 sidecar

### 8.6.2 总体流程图

```text
聊天上传参考图
-> 构建分析任务
-> 统一 canonical preview
-> 风格理解与多参考融合
-> 目标图语义区域分析
-> 全局参数预测
-> 局部区域参数预测
-> 质量护栏与风险评估
-> 生成 sliderMapping / localRegions / LUT / curves / HSL
-> 应用到当前编辑状态
-> GPU 高分辨率渲染预览
-> 用户确认后写入 sidecar
```

### 8.6.3 分步说明

#### Step 1. 前端接收聊天输入

聊天窗口负责收集：

- 主参考图文件
- 辅助参考图文件
- 当前目标图路径
- 模式选择
- 强度与附加选项

前端将其整理为统一请求：

```ts
type StyleTransferRequest = {
  targetImagePath: string;
  mainReferencePath: string;
  auxReferencePaths: string[];
  mode: "safe" | "strong";
  strength: number;
  enableLocalRegions: boolean;
  enableAuxFusion: boolean;
  writeSidecarOnApply: boolean;
};
```

#### Step 2. 构建任务上下文与缓存签名

运行时先不直接跑模型，而是先生成任务签名：

- 目标图文件签名
- 主参考图签名
- 辅助参考图签名
- 当前模式
- 当前算法版本
- 当前模型版本

若命中缓存，则直接复用：

- canonical preview
- 风格 embedding
- 语义区域结果
- 上次分析结果

这一步的意义是：

- 避免重复解 RAW
- 避免重复跑 embedding
- 避免反复跑区域识别

#### Step 3. 生成 canonical preview

这是 V4 的第一关键步骤。

对目标图：

- 若目标图是 RAW：
  - 读取 RAW 原始数据
  - 执行黑电平校正
  - 执行白平衡
  - 执行 demosaic
  - 转到统一分析色彩空间
  - 生成低分辨率 `canonical RAW preview`
- 若目标图是常规格式：
  - 正常解码
  - 做统一归一化
  - 转到统一分析色彩空间
  - 生成低分辨率分析图

对参考图：

- 主参考图与辅助参考图都统一进入：
  - decode
  - orientation normalize
  - color normalize
  - resize to analysis resolution

这一步的输出是：

- `targetPreview`
- `mainRefPreview`
- `auxRefPreviews[]`

注意：

- 这一步只用于分析
- 不会修改最终原图分辨率

#### Step 4. 参考图风格理解

这是 V4 的第二关键步骤。

对主参考图与辅助参考图：

- 使用 `ViT-B` 级别冻结视觉 backbone 提取 style embedding
- 提取全局风格向量
- 识别 scene / tonal / color / subject distribution
- 估计参考图的风格可靠度与可迁移度

主参考图负责：

- 定义主风格方向

辅助参考图负责：

- 补充色彩、影调、局部倾向

融合策略：

- 主参考图占主导权重
- 辅助参考图按与目标图的相似度、风格一致性、局部可迁移度分配权重

输出：

- `mainStyleEmbedding`
- `auxStyleEmbeddings[]`
- `fusedStyleIntent`

#### Step 5. 目标图内容理解与区域分析

对目标图 `targetPreview` 做内容分析：

- scene classification
- subject / sky / skin / vegetation / background 区域识别
- 高光 headroom 检查
- shadow density 检查
- 肤色安全区域检测
- 局部纹理与细节强度估计

优先复用现有项目基建：

- `ai-subject`
- `ai-sky`
- `ai-depth`
- 现有 mask 体系

输出：

- `targetSceneProfile`
- `targetSemanticRegions`
- `targetSafetyProfile`

#### Step 6. 全局风格映射预测

用融合后的风格意图和目标图分析结果，预测全局编辑参数：

- exposure
- contrast
- highlights
- shadows
- whites
- blacks
- temperature
- tint
- vibrance
- saturation
- dehaze
- clarity
- structure

同时预测：

- RGB / Luma curves
- HSL 偏移
- basis LUT 权重

输出：

- `globalAdjustments`
- `curves`
- `hsl`
- `globalLutPlan`

#### Step 7. 局部区域风格映射预测

再结合目标图区域信息与参考图风格意图，对局部区域分别预测：

- skin 区域
- sky 区域
- vegetation 区域
- subject 区域
- background 区域

每个区域预测：

- 是否需要处理
- 置信度
- 局部参数块
- 局部 LUT 增益
- 安全限制

输出：

- `localRegions[]`

#### Step 8. 质量护栏与风险判断

系统不会直接把模型输出原封不动交给用户，而是进入质量护栏模块。

护栏职责：

- 肤色保护
- 高光保护
- 饱和度过冲保护
- 参考图跨题材风险评估
- 模式约束

模式差异：

- `safe`：
  - 更强限制
  - 更保守参数
  - 更弱局部风格注入
- `strong`：
  - 更激进靠近参考图
  - 仍保留底线护栏
  - 若高风险则提示，但默认继续运行

输出：

- `guardedGlobalAdjustments`
- `guardedLocalRegions`
- `qualityReport`
- `riskWarnings`

#### Step 9. 生成结构化编辑结果

到这一步，系统已经不再返回“黑箱结果图”，而是返回结构化编辑语义。

标准输出：

- `globalAdjustments`
- `curves`
- `hsl`
- `globalLut`
- `localRegions`
- `qualityReport`
- `styleDebug`
- `sliderMapping`

其中：

- `sliderMapping` 负责把结果映射到现有滑块
- `localRegions` 负责把结果映射到现有局部蒙版系统
- `styleDebug` 负责解释“为什么这么建议”

#### Step 10. 应用到当前编辑状态

如果用户点击应用，系统不会生成一张新图覆盖原图，而是：

- 把全局建议写入当前 adjustments 状态
- 把曲线写入曲线对象
- 把 HSL 写入颜色对象
- 把 LUT 计划转成当前可用 LUT 引用或参数
- 把局部区域写入 mask container / sub-mask 调整块

这一阶段依然完全保留：

- 滑块可调
- 局部可开关
- 局部可回显
- 用户可继续手工覆盖

#### Step 11. GPU 高分辨率应用与预览渲染

真正的高分辨率效果图，不由大模型直接生成，而由现有编辑渲染链完成：

- 原始分辨率目标图进入现有 GPU / shader 渲染管线
- 应用全局参数
- 应用 curves
- 应用 HSL
- 应用 LUT
- 应用局部 mask 参数

得到：

- 高分辨率预览结果
- 导出时的高质量最终结果

这一步是 V4 能同时保住：

- 速度
- 原图画质
- 可编辑语义

的核心原因。

#### Step 12. 写入 sidecar

若用户确认应用，则将分析结果写入 sidecar：

- 输入签名
- 主参考图签名
- 辅助参考图签名
- 当前模式
- 当前模型版本
- globalAdjustments
- curves
- hsl
- globalLut
- localRegions
- sliderMapping
- riskWarnings

这样可以保证：

- 可复现
- 可撤销
- 可继续编辑
- 可缓存

### 8.6.4 端到端伪代码

下面给出一版足够贴近实现的主流程伪代码。

```rust
fn run_style_transfer(req: StyleTransferRequest) -> StyleTransferResult {
    let task_sig = build_task_signature(&req);

    if let Some(cached) = load_cached_style_result(&task_sig) {
        return cached;
    }

    let target_preview = build_canonical_preview(req.target_image_path);
    let main_ref_preview = build_reference_preview(req.main_reference_path);
    let aux_ref_previews = req
        .aux_reference_paths
        .iter()
        .map(build_reference_preview)
        .collect::<Vec<_>>();

    let target_regions = analyze_target_regions(&target_preview);
    let target_scene = analyze_target_scene(&target_preview, &target_regions);
    let target_safety = analyze_target_safety(&target_preview, &target_regions);

    let main_style = encode_style_embedding(&main_ref_preview);
    let aux_styles = aux_ref_previews
        .iter()
        .map(encode_style_embedding)
        .collect::<Vec<_>>();

    let fused_style = fuse_reference_styles(
        main_style,
        aux_styles,
        &target_scene,
        req.enable_aux_fusion,
    );

    let global_plan = predict_global_mapping(
        &fused_style,
        &target_preview,
        &target_scene,
        req.mode,
        req.strength,
    );

    let local_plan = if req.enable_local_regions {
        predict_local_region_mapping(
            &fused_style,
            &target_preview,
            &target_regions,
            req.mode,
            req.strength,
        )
    } else {
        vec![]
    };

    let guarded = apply_quality_guard(
        global_plan,
        local_plan,
        &target_safety,
        &target_regions,
        req.mode,
    );

    let slider_mapping = build_slider_mapping(&guarded);
    let debug_info = build_style_debug(&guarded, &fused_style, &target_scene);

    let result = StyleTransferResult {
        global_adjustments: guarded.global_adjustments,
        curves: guarded.curves,
        hsl: guarded.hsl,
        global_lut: guarded.global_lut,
        local_regions: guarded.local_regions,
        quality_report: guarded.quality_report,
        style_debug: debug_info,
        slider_mapping,
        risk_warnings: guarded.risk_warnings,
    };

    save_cached_style_result(&task_sig, &result);
    result
}
```

应用阶段伪代码：

```rust
fn apply_style_transfer_to_editor(
    result: &StyleTransferResult,
    current_adjustments: &mut Adjustments,
    target_full_res_image: &ImageHandle,
) {
    apply_global_adjustments(current_adjustments, &result.global_adjustments);
    apply_curves(current_adjustments, &result.curves);
    apply_hsl(current_adjustments, &result.hsl);
    apply_global_lut(current_adjustments, &result.global_lut);
    apply_local_regions_as_masks(current_adjustments, &result.local_regions);
    apply_slider_mapping_metadata(current_adjustments, &result.slider_mapping);

    render_high_res_preview_with_gpu(
        target_full_res_image,
        current_adjustments,
    );
}
```

### 8.6.5 一句话总结

V4 的真正主链不是：

- 上传参考图
- 大模型直接生成最终图

而是：

- 上传参考图
- 小中型模型在低分辨率上理解风格与内容
- 产出结构化编辑参数
- 用现有高分辨率编辑引擎去应用结果

这就是它能同时兼顾：

- 风格迁移质量
- 等待时间
- 原图画质
- 滑块可调
- sidecar 可复现

的根本原因。

---

# 九、输出语义设计

## 9.1 V4 的标准输出

分析式风格迁移应输出如下结构：

- `globalAdjustments`
- `curves`
- `hsl`
- `globalLut`
- `localRegions`
- `qualityReport`
- `styleDebug`
- `sliderMapping`

## 9.1.1 sliderMapping 的语义

`sliderMapping` 用于把分析结果显式映射到现有的调试滑块和参数系统。

它至少应包含：

- 当前建议对应的 adjustment key
- 建议值
- 建议来源
- 是否属于高风险建议
- 用户是否已手动覆盖

这样设计的目的不是增加一层重复结构，而是保证：

- 分析结果与当前 UI 调试模式保持一致
- 用户能看到“系统为什么这么推”
- 用户能直接基于当前滑块继续微调

## 9.2 localRegions 的语义

`localRegions` 中的每个对象至少应包含：

- region type
- mask source
- confidence
- recommended adjustments
- optional local LUT strength
- safety notes

例如：

- skin 区域建议温度、色调、亮度微调
- sky 区域建议蓝青通道 HSL 偏移
- vegetation 区域建议绿色亮度和饱和度收敛

## 9.3 与现有编辑系统的关系

分析式输出必须保持可编辑：

- 用户可直接应用
- 用户可局部关闭
- 用户可调整强度
- 用户可只应用全局，不应用局部
- 用户可在现有调试滑块上继续微调

不能把分析式结果变成一键黑箱结果。

## 9.4 Sidecar 持久化要求

本轮讨论已确认，分析式风格迁移结果必须写入 sidecar。

写入对象至少应包括：

- 全局参数建议
- 曲线
- HSL
- LUT 引用或参数
- 局部区域参数
- `sliderMapping`
- 当前运行模式
- 参考图签名
- 算法版本

其目的在于：

- 让结果可复现
- 让结果可撤销
- 让结果可继续微调
- 为缓存和后续分析闭环提供稳定锚点

---

# 十、数据与训练策略

## 10.1 数据来源

V4 推荐采用三类数据。

### A. 公共数据集

- MIT-Adobe FiveK
- PPR10K
- 其他可合法使用的摄影调色数据

### B. 自建风格对

将内部摄影样本和参考图整理成：

- 当前图
- 参考图
- 期望输出参数
- 可选的目标图

### C. 合成数据

基于已有 preset 和参数系统批量生成训练对：

- 原图
- preset
- 输出图
- 参数标签

这种数据虽然不等于真实摄影师风格迁移，但很适合训练基础参数头和 LUT 头。

## 10.2 训练目标

V4 不建议只训练“输出图像误差”。

应采用多头目标：

- 参数回归损失
- LUT 损失
- 曲线损失
- 区域风格一致性损失
- 肤色保护损失
- 高光惩罚损失
- 风格接近度排序损失

## 10.3 数据闭环

产品上线后应建立闭环：

- 用户是否应用建议
- 用户调整了哪些参数
- 用户撤销了哪些区域建议
- 哪些场景经常被判定“不像参考图”

这些行为数据不一定直接用于在线学习，但应该进入离线评测与下一轮模型更新。

---

# 十一、质量评测体系

## 11.1 客观指标

至少建立以下指标：

- 全局风格接近度
- 肤色误差
- 高光溢出风险
- 饱和度过冲风险
- 局部区域一致性
- 参数可用率

## 11.2 主观评测

必须建立小规模人工评审集，至少覆盖：

- 人像
- 风光
- 城市夜景
- 室内低照度
- 高动态范围

评审问题建议包括：

- 是否接近参考图风格
- 是否保留摄影照片质感
- 是否有局部区域翻车
- 是否愿意直接作为调色起点

## 11.3 产品指标

除了图像指标，还必须看：

- 用户应用率
- 二次调节幅度
- 撤销率
- 局部建议关闭率
- 耗时

对于分析式主路径，V4 仍应坚持：

- 常规分析 1 到 5 秒
- 超过 10 秒视为异常
- 超过 2 分钟必须转入“等待风险确认”状态

## 11.4 预计落地收益评估

本节结论属于工程估算，不代表已经完成实测 benchmark。

估算依据包括：

- 当前仓库现有实现路径
- 当前风格迁移模块的输入与特征提取方式
- 当前项目已经具备的 `ort + wgpu + mask + shader + sidecar` 基建
- 已调研公开路线在任务定义与运行形态上的适配程度

### 11.4.1 当前基线判断

当前分析式实现的真实特点是：

- 风格分析优先依赖 embedded preview 或普通图像解码
- 特征提取被缩到约 `600px`
- 已经具备 feature mapping、auto refine、curves、3D LUT 等能力
- 但主导仍然是全局近似，而不是稳定的局部区域迁移

因此，当前版本的优势与短板很明确：

- 优势：快、轻、容易解释
- 短板：更像“全局调色建议”，不像“摄影风格迁移”

### 11.4.2 质量变化预估

如果按 V4 当前方案完整、安全落地，质量变化不会是“小修小补”，而会是明显的代际提升。

最重要的变化是：

- 从“全局相似”升级到“全局 + 局部相对一致”
- 从“多数时候只能给起点”升级到“相当一部分场景可直接作为可用结果或少量微调结果”
- 从“跨题材容易发脏”升级到“跨题材至少能稳定给出弱建议或可控强建议”

按典型题材估算：

- 人像 / 婚礼 / 群像：
  - 肤色翻车率预计可下降约 `40% - 60%`
  - 背景被人物风格误伤的情况预计可下降约 `30% - 50%`
- 风光 / 城市风光 / 旅拍：
  - 天空、植被、建筑互相串色的情况预计可下降约 `35% - 55%`
  - 高光与阴影两端被全局硬拉导致不自然的情况预计可下降约 `30% - 45%`
- 室内 / 夜景：
  - 混合光源下整体偏色失控的情况预计可下降约 `25% - 40%`
  - 霓虹、暖灯、高光边缘被污染的情况预计可下降约 `20% - 35%`

按整体可用性估算：

- “可直接应用或只需少量微调”的结果占比，预计可从当前约 `20% - 35%` 提升到 `55% - 75%`
- “明显翻车、用户基本不会采用”的结果占比，预计可从当前约 `15% - 30%` 降到 `5% - 10%`

这些数字的本质不是说模型会变得“完美”，而是：

- 它会从“有时有帮助的规则建议器”
- 变成“多数主流题材下可作为正式风格迁移起点的分析式系统”

### 11.4.3 速度变化预估

在速度上，V4 完整版不会比当前分析式更快。

真实判断应是：

- 相比当前分析式，分析阶段会变慢
- 但绝不会接近生成式的 `30 分钟` 级等待
- 相比生成式失败路线，仍然是数量级更快

原因很明确：

- 规范 RAW 预览会比直接拿 embedded preview 更慢
- 语义区域识别和 style understanding backbone 会引入额外推理
- 但这些推理都发生在低分辨率分析图上
- 高分辨率结果仍由现有 GPU 编辑链应用，不是让大模型逐像素重算 6000 万像素

因此，时间代价的增长主要来自“分析层”，不是“最终应用层”。

### 11.4.4 典型耗时区间预估

以下为完整方案落地后的工程估算区间。

#### A. 热启动、单参考图、常规题材

- 高性能机器：约 `3 - 8 秒`
- 主流中端机器：约 `6 - 15 秒`
- 低配机器：约 `15 - 40 秒`

#### B. 冷启动、单参考图、60MP RAW、强风格模式

- 高性能机器：约 `8 - 20 秒`
- 主流中端机器：约 `15 - 35 秒`
- 低配机器：约 `30 - 90 秒`

#### C. 多参考图、复杂题材、冷启动

- 高性能机器：约 `15 - 30 秒`
- 主流中端机器：约 `25 - 60 秒`
- 低配机器：约 `45 - 120 秒`

说明：

- 超过 `120 秒` 的情况不应作为常态
- 一旦进入该区间，必须触发“继续等待风险确认”
- 真正把系统拖到分钟级的，不应是高分辨率应用本身，而应是：
  - 首次模型初始化
  - 多参考图特征融合
  - 慢速 RAW 解码
  - 低配 CPU 上的 canonical preview 构建

### 11.4.5 与当前实现的速度关系

如果只和当前分析式相比，V4 完整版大概率会慢约：

- `2x - 5x` 的分析时延

但如果和你已经验证失败的生成式路线相比，V4 完整版大概率会快：

- `10x - 100x+`

所以速度结论不是“更快了”，而是：

- 用可接受的秒级到几十秒级成本
- 换来明显更高的风格迁移质量
- 同时保住原图画质、可编辑性和跨平台产品形态

### 11.4.6 为什么不会再次回到 30 分钟级

V4 之所以大概率不会重演生成式的超长等待，核心原因有四个：

1. 不做高分辨率生成式逐步去噪。
2. 不让神经网络直接输出最终 6000 万像素图像。
3. 大模型只负责理解风格，小图上运行。
4. 最终高分辨率结果由现有 shader / LUT / mask 编辑链负责应用。

因此，它的复杂度更接近：

- `低分辨率理解 + 高分辨率参数化应用`

而不是：

- `高分辨率内容重生成`

### 11.4.7 最现实的产品判断

如果你问“完整安全落地后，最现实的变化是什么”，我的判断是：

- 质量上：会从“分析式可用但不够像参考图”提升到“多数主流题材下已经像一个正式的、可编辑的风格迁移工具”
- 速度上：会比当前分析式变慢，但仍稳定处于摄影软件用户可以接受的等待区间，大幅优于生成式失败路线
- 产品上：这是一次很值得的交换，因为换来的不是单纯画面更像，而是更像、还保留滑块语义、还保留原图质量、还保留 sidecar 可复现

## 11.5 必须补的验证

为避免主观乐观，V4 在进入正式开发前必须补以下 benchmark：

- `embedded preview` vs `canonical RAW preview` 的耗时对比
- 单参考图 vs 多参考图的耗时对比
- 热启动 vs 冷启动的耗时对比
- `ViT-B + 小头 + LUT 执行器` 的端到端耗时
- 各题材下的“直接可用率 / 微调后可用率 / 放弃率”

---

# 十二、实施路线

## Phase 1：规范输入 + 局部止血

目标：

- 在不大改 UI 的前提下，让分析式结果先明显更稳
- 建立“低分辨率分析、高分辨率应用”的清晰边界
- 保证结果仍然完全映射到现有滑块系统
- 跑通“常规格式参考图 -> 原厂 RAW 当前图”的主路径
- 建立安全起点模式与强风格迁移模式的双策略框架

工作项：

- 建立 `canonical input` 入口
- 建立参考图格式适配层，统一 `jpg/jpeg/png/tiff` 参考图分析口径
- 减少对 embedded preview 的强依赖
- 保留当前 feature mapping / curves / HSL / LUT
- 增加 skin / sky 两类轻量区域识别
- 为 skin / sky 输出小幅局部参数建议
- 新增 `sliderMapping` 输出结构
- 建立输入签名缓存与慢任务诊断
- 增加超时后的继续等待确认交互
- 增加自动局部蒙版默认隐藏、可回显机制
- 增加主参考图 + 辅助参考图的数据结构
- 增加 sidecar 写回结构
- 把 `expert_presets` 从静态标签匹配升级为“打分排序”
- 建立最小人工评测集

阶段结果：

- 人像与风光两类场景的失败率明显下降
- 当前分析式从“全局近似”提升到“有基础区域意识”

## Phase 2：学习型映射核心版

目标：

- 把分析式从规则主导升级到“规则 + 学习型映射”主导

工作项：

- 引入 basis LUT 或 image-adaptive LUT
- 引入 bilateral grid / local affine 映射层
- 完成 subject / vegetation / background 区域特征
- 建立 preset embedding / style prototype
- 输出全局参数 + 曲线 + HSL + 局部参数块的联合结果
- 建立回归测试集与自动评测脚本

阶段结果：

- 局部风格迁移质量显著提升
- 对参考图 look 的还原能力明显强于当前纯规则实现
- 仍保持分析式可编辑语义

## Phase 3：学习型 preset 与产品闭环版

目标：

- 建立真正可持续迭代的分析式风格迁移系统

工作项：

- 离线训练 preset predictor
- 建立用户行为闭环
- 引入风格原型库与场景库
- 支持不同题材专用映射头
- 引入更精细的区域冲突求解
- 优化局部建议在 UI 中的可视化与开关逻辑

阶段结果：

- 分析式路线形成长期可迭代能力
- 效果提升不再依赖不断手写规则

---

# 十三、代码层面的重构建议

## 13.1 Rust 侧重构建议

建议逐步把当前大文件：

- `src-tauri/src/style_transfer.rs`

拆成：

- `src-tauri/src/style_transfer/mod.rs`
- `src-tauri/src/style_transfer/canonical_input.rs`
- `src-tauri/src/style_transfer/features.rs`
- `src-tauri/src/style_transfer/semantic_regions.rs`
- `src-tauri/src/style_transfer/global_mapping.rs`
- `src-tauri/src/style_transfer/local_mapping.rs`
- `src-tauri/src/style_transfer/preset_ranker.rs`
- `src-tauri/src/style_transfer/quality_guard.rs`
- `src-tauri/src/style_transfer/evaluator.rs`

## 13.2 前端侧建议

前端不需要先重做工作流，只要先支持：

- 展示当前分析来源
- 展示全局建议
- 展示局部建议
- 展示结构化 `sliderMapping`
- 支持逐项应用
- 支持整体强度滑杆
- 支持关闭局部区域建议
- 支持在现有调试滑块上继续覆盖系统建议

## 13.3 Python 侧建议

当前 `python/style_transfer_service` 不再适合作为产品主链路。

建议未来拆分为：

- `python/style_transfer_training`
- `python/style_transfer_eval`
- `python/style_transfer_tools`

避免继续保留“服务入口仍然是生成式残留”的语义混乱。

---

# 十四、风险与边界

## 14.1 主要风险

- 语义区域识别不准，导致局部建议误伤
- 学习型 LUT 过强，破坏可编辑参数语义
- 数据集风格偏差，导致结果过拟合某类摄影审美
- RAW 规范输入做不好，导致模型学习建立在不稳定输入上
- 本地模型体积与部署成本可能影响可安装性与冷启动体验

## 14.2 应对原则

- 所有学习型模块输出都必须可降级
- 局部建议必须可以单独关闭
- 质量护栏必须独立存在
- 数据更新必须走离线评测后再上线
- 本地模型引入前必须先完成体积、效果和部署代价调研

---

# 十五、最终结论

V4 的结论不是：

```text
分析式不行，只能回到生成式
```

而是：

```text
当前分析式之所以效果不理想
不是因为“分析式这条产品路线错了”
而是因为它还停留在
“全局规则型映射 + 少量经验预设”的阶段
```

RapidRAW 更适合走的路线是：

- 坚持分析式主路径
- 统一 RAW 分析输入域
- 增加语义区域能力
- 引入学习型 LUT / local affine / preset predictor
- 继续把输出约束在可编辑参数系统中

这条路线的价值在于：

- 保留专业摄影工作流语义
- 保留速度优势
- 保留可解释性
- 同时把效果质量继续往上推

---

# 十六、一句话总结

```text
RapidRAW V4 不再讨论“生成式还是分析式”
而是明确选择分析式主路径
并把它升级成“规范输入、语义分区、学习型映射、可编辑回写”的专业风格迁移系统
```

---

# 十七、参考方向

- Neural Preset, CVPR 2023
  - https://arxiv.org/abs/2303.13511
  - https://github.com/ZHKKKe/NeuralPreset
- HDRNet
  - https://github.com/google/hdrnet
- Image-Adaptive-3DLUT
  - https://github.com/HuiZeng/Image-Adaptive-3DLUT
- LUTwithBGrid
  - https://github.com/WontaeaeKim/LUTwithBGrid
- SVDLUT
  - https://github.com/WontaeaeKim/SVDLUT
- Deep Preset
  - https://github.com/minhmanho/deep_preset
- MIT-Adobe FiveK
  - https://data.csail.mit.edu/graphics/fivek/
- PPR10K
  - https://arxiv.org/abs/2105.09180

---

# 十八、讨论决策记录

## 18.1 维护要求

本节用于记录每轮讨论后已经形成但尚未完全工程化落地的决策。

记录原则：

- 已定共识：应尽快上升到正式章节
- 待验证事项：必须保留验证条件
- 待调研事项：必须保留后续行动

## 18.2 本轮记录（上一轮讨论沉淀）

### A. 统一 RAW 规范预览链路

当前共识：

- 若统一 `canonical RAW preview` 相比现状在效果与可靠性上均为正向收益，则允许切换主路径
- 是否切换不能只凭直觉，必须补 benchmark

待验证问题：

- 比当前 embedded preview 路径慢多少
- 是否仍处于用户可接受等待区间
- 对结果可靠性提升多少
- 对风格接近度提升多少

### B. 低配机器的超时处理

当前共识：

- 修图软件应把质量放在第一顺位
- 对低配机器，不应为了强行追求快而默认牺牲质量
- 超时后只需提示长时间等待风险，由用户决定是否继续

### C. 双模式策略

当前共识：

- 分析式至少支持两套策略模式
- 安全起点模式与强风格迁移模式均应存在

### D. 自动蒙版回显策略

当前共识：

- 自动生成的局部蒙版默认隐藏
- 但必须支持用户主动开启回显
- 必须支持二次修改

### E. 多参考图策略

当前共识：

- 多参考图采用“主参考图 + 辅助参考图”结构
- 主参考图必传且唯一
- 辅助参考图可不传，也可多传

### F. 首版能力边界

当前共识：

- 首版只处理色彩、影调、HSL、曲线、局部色彩偏移
- grain、halation、glow、vignette 不进入首版主路径
- 这些效果列入后续功能

### G. 本地模型

当前状态：

- 用户接受引入本地模型
- 但在决定前，需要先完成“最强可行方案 + 模型体积 + 部署代价”的调研

### H. RAW 兼容范围

当前共识：

- RAW 兼容范围以现有基建实际支持为准
- 二次开发层不额外重定义支持矩阵

### I. 跨题材强迁移策略

当前共识：

- 当系统判断存在高风险时，默认仍按强迁移运行
- 但必须明确提示风险

### J. Sidecar 写回

当前共识：

- 风格迁移结果必须写入 sidecar
- 结果需要支持复现、撤销和继续编辑

## 18.3 本轮记录（本地模型路线调研）

### K. 本地模型路线综合判断

当前共识：

- 当前最接近“参考图驱动分析式风格迁移”的公开路线是 `Neural Preset`
- 当前最适合作为本地高分辨率执行底座的公开路线是 `SVDLUT / 3D LUT` 家族
- V4 不应二选一，而应采用“风格理解器 + 高速执行器”的混合架构

### L. 当前仓库接入结论

当前共识：

- 当前仓库已具备 `ONNX Runtime` 接入基础
- 当前仓库已具备模型下载、缓存、校验和 Session 复用能力
- 当前仓库已具备 `wgpu` 与 shader 高分辨率执行能力
- 因此 V4 本地模型主链应优先复用现有 `ort + GPU` 基建

### M. 运行时模型接入原则

当前共识：

- 运行时优先接入可导出 `ONNX` 的小模型
- 运行时不应把论文中的 PyTorch 自定义扩展原样作为硬依赖
- 高分辨率照片不应通过深层神经网络逐像素重算主输出
- 小模型主要负责预测结构化编辑结果，高分辨率应用仍交给现有编辑引擎

### N. 待拍板问题

本轮讨论后，该组问题已完成拍板：

- 本地模型应随安装包内置
- `2GB` 以内的模型总包体积可接受
- 是否做成可选模型包，取决于最终实际体积与工程收益，而不是预设为必须拆包

## 18.4 本轮记录（模型分发与 2GB 预算）

### O. 模型分发策略

当前共识：

- 风格迁移本地模型应随安装包内置
- 不默认采用首次运行时下载

### P. 模型体积上限

当前共识：

- 若模型总包体积在 `2GB` 以内，可以接受内置
- “是否做成可选模型包”主要取决于最终实际体积与工程收益

### Q. 2GB 预算下的技术判断

当前共识：

- 即使模型预算放宽到 `2GB`，V4 的总架构方向仍不改变
- 更大的预算主要用于提升“风格理解能力”，不用于重新引入重生成式主链
- `SA-LUT` 在 2GB 预算下变得更值得研究，但更适合作为 teacher / benchmark / 蒸馏参考
- 运行时主链仍应坚持：
  - 结构化输出
  - 高分辨率执行器
  - 可编辑滑块映射
  - sidecar 可复现

## 18.5 本轮记录（由系统代为拍板的技术决策）

### R. Phase 1 运行时主链

当前结论：

- Phase 1 正式坚持纯跨平台 `ONNX Runtime` 主链
- 不为 `CUDA-only` 研究模型开正式运行时例外
- `CUDA-only` 模型只用于 teacher、benchmark、蒸馏参考或离线实验

### S. 首版风格理解 backbone

当前结论：

- 首版默认选择 `ViT-B` 级别中型冻结视觉 backbone
- 不首发同时维护 `ViT-B / ViT-L` 双档运行时产品方案
- 架构保留后续升级到 `ViT-L` 的空间，但必须先通过 benchmark

## 18.6 本轮记录（完整落地后的质量与速度预估）

### T. 质量预估

当前结论：

- 完整落地后的提升将是代际提升，而不是小修小补
- 重点收益来自：
  - canonical RAW input
  - 区域级风格迁移
  - 学习型风格理解
  - 高分辨率参数化应用

### U. 速度预估

当前结论：

- V4 完整版相较当前分析式会变慢
- 但相较生成式失败路线，仍然保持数量级优势
- 工程估算上：
  - 常规热启动多在秒级到十几秒级
  - 复杂冷启动场景多在几十秒级
  - 超过 `2 分钟` 不应成为常态

### V. 必测 benchmark

当前结论：

- 在正式开发前，必须补足端到端 benchmark
- 否则任何“质量提升”和“速度可接受”的判断都只能视为工程估算，而不是已验证事实

## 18.7 本轮记录（端到端产品主流程）

### W. 从聊天上传到最终应用的主链

当前结论：

- 主参考图负责定义主风格方向
- 辅助参考图负责补充风格信息
- 所有参考图先进入统一 preview 与风格理解链
- 目标图先进入 canonical preview 与区域分析链
- 输出必须先成为结构化编辑结果，再交给现有高分辨率编辑引擎应用

### X. 运行时设计原则

当前结论：

- 模型只负责低分辨率理解与参数预测
- 最终高分辨率效果由现有 GPU / shader / LUT / mask 编辑链完成
- 风格迁移结果不是黑箱结果图，而是可编辑的滑块、曲线、HSL、LUT、局部区域参数

## 18.8 本轮记录（工程实施状态核对）

### Y. 已完成的前置工程

当前结论：

- 已建立统一的风格迁移模型管理入口，继续沿用 `~/.qraw/models` 作为共享模型仓
- 已在前端加入：
  - `safe / strong` 策略切换
  - 主参考图 + 辅助参考图请求结构
  - 模型准备按钮
  - 模型就绪状态卡片
- 已在运行时加入：
  - 多参考图字段
  - 策略模式字段
  - 模型状态回传结构

### Z. 当前未完成项

当前结论：

- `style_transfer_dinov2_vitb.onnx`
  - 当前仍未正式落地到共享模型仓
- `style_transfer_dinov2_vitb.preprocess.json`
  - 当前仍未正式落地到共享模型仓
- 当前运行时主链仍在调用既有 `style_transfer::analyze_style_transfer`
- 新增的模型管理目前主要完成了“状态管理 / 准备入口 / UI 暴露”，尚未完成 DINOv2 风格理解模型的正式推理接入

### AA. 对“是否严格符合文档”的阶段判断

当前结论：

- 当前 staged 代码不应被视为“100% 严格符合 V4 文档”
- 更准确地说：
  - 已完成 Phase 1 的管理骨架与交互骨架
  - 尚未完成 V4 文档要求的核心模型推理闭环
  - 尚未完成 canonical RAW preview + 风格 backbone + 参数预测头 + 高分辨率应用闭环

## 18.9 本轮记录（模型下载与运行时对接收口）

### AB. 风格 backbone 模型来源与落盘结果

当前结论：

- `style_transfer_dinov2_vitb.onnx`
  - 已正式落盘到共享模型仓 `~/.qraw/models`
- `style_transfer_dinov2_vitb.preprocess.json`
  - 已正式落盘到共享模型仓 `~/.qraw/models`
- 当前使用的真实上游来源不是 `CyberTimon/RapidRAW-Models`
  - 而是 `onnx-community/dinov2-base-ONNX`
- 项目内继续沿用 RapidRAW 侧的目标工件命名：
  - `style_transfer_dinov2_vitb.onnx`
  - `style_transfer_dinov2_vitb.preprocess.json`

本轮实测 SHA256：

- `style_transfer_dinov2_vitb.onnx`
  - `f16115e628d65b7cc7b1e16c504e2af682169aabf3fff4edfe906118f522e204`
- `style_transfer_dinov2_vitb.preprocess.json`
  - `14e780d86fa1861f8751f868d7f45425b5feb55c38ca26f152ca5097ab30f828`

### AC. 运行时主链对接结果

当前结论：

- `prepare_style_transfer_models`
  - 已把风格 backbone 作为必需模型纳入准备链路
- `get_style_transfer_model_status`
  - 已把风格 backbone 与 preprocess 配置纳入状态返回
- `analyze_style_transfer`
  - 已强制加载风格 backbone
- 风格 backbone 输出 embedding 已接入：
  - 主参考图与目标图语义相似度计算
  - 辅助参考图加权融合
  - 自适应 early-exit / VLM 触发阈值
  - 风险提示生成
  - 质量报告备注

这意味着当前已不再是“只有模型管理壳子、没有真实推理接入”的状态。

### AD. 可重复下载与本地导出兼容性修正

当前结论：

- `scripts/download_style_transfer_models_partial.sh`
  - 已补入风格 backbone 与 preprocess 配置的下载和 SHA 校验
- `scripts/export_style_transfer_backbone.py`
  - 已修正输出的 `preprocess.json` 字段命名
  - 保证其与 Rust 运行时的反序列化结构兼容
- Rust 运行时侧也已补充 preprocess 字段别名兼容
  - 允许同时读取 snake_case / camelCase 风格配置

### AE. 工程验证结果

当前结论：

- `cargo test extract_style_transfer_embedding_smoke_test -- --nocapture`
  - 已通过
- 该 smoke test 证明：
  - 当前 Rust + ONNX Runtime 代码可以真实加载 `style_transfer_dinov2_vitb.onnx`
  - 可以读取 `style_transfer_dinov2_vitb.preprocess.json`
  - 可以对测试图像产出 L2 归一化 embedding
- `cargo check`
  - 已通过
- `pnpm build`
  - 已通过
- `scripts/download_style_transfer_models_partial.sh`
  - 已通过本地复核，现有模型文件全部 hash 校验成功

### AF. 与文档一致性的仓库修正

当前结论：

- 仓库内 `.qraw-models/*` 二进制副本已移除
- 当前工程只保留共享模型仓 `~/.qraw/models` 作为正式模型落点
- `get_qraw_models_dir`
  - 已取消隐式工作区 dev fallback
  - 仅保留 `QRAW_MODELS_DIR` 显式覆盖和 `~/.qraw/models` 默认落点
- 这与 V4 文档中“模型集中管理、避免仓库内携带运行时模型副本”的要求保持一致

### AG. 对本轮完成度的正式判断

当前结论：

- “模型下载成功并和代码逻辑对接成功”这一轮目标已经成立
- 本轮已完成：
  - 模型真实下载
  - hash 校验
  - 模型状态管理
  - 运行时真实加载
  - ONNX 推理自检
  - 多参考图语义加权接入
  - UI 准备与状态暴露
  - sidecar 结果承接
- 但 V4 文档中的后续增强项仍然存在：
  - 更强的学习型参数预测头
  - 更细的局部区域策略
  - benchmark 数据闭环

这些属于后续 Phase 2 / Phase 3 增强，不影响本轮“模型闭环已落地”的判断。

## 18.10 本轮记录（主辅参考图确认、处理调试与参数同源）

### AH. CLIP Tokenizer 的定位澄清

当前结论：

- `CLIP Tokenizer` 不属于当前风格迁移主链的必需模型
- 它当前属于可选辅助能力
  - 主要服务于其他 AI 能力或可选辅助链路
- 即使 `CLIP Tokenizer` 未就绪
  - 也不应阻塞分析式风格迁移主流程
  - 也不应被用户误解为“主风格迁移模型未就绪”

因此本轮已修正 UI 表达：

- 必需模型与可选模型必须显式区分
- 可选模型未就绪时，需明确标注“不影响主流程”

### AI. 主参考图与辅助参考图确认交互

当前结论：

- 参考图导入后，不应立即启动风格迁移
- 必须先在聊天窗口中展示：
  - 主参考图预览
  - 辅助参考图预览
  - 明确的确认 / 取消入口
- 用户确认后，才正式进入风格迁移分析流程

本轮已落地：

- 新增聊天窗口内的参考图确认卡
- 主参考图与辅助参考图已区分展示
- 该确认步骤先于 `run_style_transfer` 执行

### AJ. “是否真的走了模型链”的可见性要求

当前结论：

- 仅靠“用户主观感觉”无法判断是否命中了增强链
- 系统必须显式输出处理调试结果，告诉用户本次执行是否真实经过：
  - canonical input
  - style backbone
  - 主辅参考图语义相似度
  - 各增强开关是否启用
  - 最终映射出的局部区域 / 滑块 / 风险提示数量

本轮已落地：

- 后端新增结构化 `processingDebug`
- 聊天回复中新增“处理调试”卡
- 调试卡提供一键复制调试数据按钮

### AK. 默认执行策略修正

当前结论：

- 聊天侧默认开启 `pure algorithm`
  - 会让增强链默认被关闭
  - 与 V4 文档“默认走完整分析增强链”的方向不一致

本轮已修正：

- 默认不再启用 `pure algorithm`
- 默认改为走完整分析增强链
- `pure algorithm` 仅作为显式降级选项保留

### AL. 调色参数命名与回显同源

当前结论：

- AI 聊天中的调色结果与右侧“调整”模块，本质上必须是同一份数据
- 风格迁移结果不能再走一套“聊天专用命名”或“聊天专用 patch”
- 必须只写回项目内置的正式调色字段，并确保侧边栏切换后回显一致

本轮已修正：

- 聊天侧结构化 patch 改为优先消费正式 `guardedGlobalAdjustments / sliderMapping`
- 仅将已知正式调色字段写回主 `Adjustments`
- `curves / hsl / LUT / masks` 继续走正式结构写回
- 聊天消息中的 `appliedValues` 也改为基于同一份结构化 patch 回填

这意味着：

- AI 聊天里的滑块
- 右侧调整面板里的滑块
- sidecar 中记录的结构化结果

现在都应收敛到同一组正式字段语义之上。
