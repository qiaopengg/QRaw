# Design QA：QRaw 智能选图全生命周期原型

- source visual truth path: `src/assets/reference/smart-culling-option-1.png`
- implementation screenshot path: `screenshots/06-review.png`
- combined comparison evidence: `screenshots/design-qa-comparison.png`
- viewport: 浏览器内部 1280 × 720；客户端截图内容区 1173 × 720
- state: 深色主题，智能选图复核页，02 仪式 / 交换戒指，相似组与受保护组合展开

## Findings

最终对照没有仍需处理的 P0、P1 或 P2 问题。

- [P3] 相似组前三张使用同一场景素材
  - Location: `ReviewScreen` / `.review-photo-row`
  - Evidence: 视觉源使用同一事件的连续瞬间；原型前三张使用同一生成素材的不同评分状态。
  - Impact: 不阻塞布局、信息层级或交互验证，但生产接入真实任务数据后应显示真实连拍差异。
  - Follow-up: 生产数据接入后按真实相似组成员和拍摄顺序渲染，不为原型额外生成虚假连拍。

## Required fidelity surfaces

- Fonts and typography: 使用 Inter / SF Pro Text / 苹方 / 微软雅黑回退，字重、字号与 Option 1 的紧凑桌面工具层级一致；小字仍保持可辨识对比度。
- Spacing and layout rhythm: 保留三栏复核结构、顶部阶段条、组卡片和右侧检查器；1280 × 720 中无持久控制被裁切或推出窗口。
- Colors and visual tokens: 深灰黑表面、细分隔线、近白主操作，以及绿/黄/红语义状态与视觉源一致。
- Image quality and asset fidelity: 使用六张本地生成的高质量婚礼摄影素材和 Lucide 图标；没有 emoji、占位框、手绘 SVG 或 CSS 假图片。
- Copy and content: 页面文案对应当前需求规则，明确离线、人工保护、未确认不写入、取消与失败语义。
- Accessibility and states: 核心按钮使用语义化 button 和可访问名称，焦点样式可见；成功、警告、失败不只依赖颜色。

## Full-view comparison evidence

`screenshots/design-qa-comparison.png` 将已选 Option 1 与浏览器渲染的复核页放入同一对照画布。
最终实现保持了视觉源的整体构图、信息密度、三栏比例、组层级和右侧详情检查结构。

## Focused region comparison evidence

同一对照图下半部分提供复核内容区焦点，检查了相似组、星级、颜色标签、受保护 HDR 组合、
失败折叠区和右侧来源/原因区域。密集 UI 的边框、间距、图标风格和状态颜色未发现 P2 以上偏差。

## Comparison history

1. Initial responsive pass
   - Finding: [P2] 1280 × 720 时关键人物大图的固有尺寸撑高网格，底部缩略图与保存按钮超出窗口。
   - Fix: 为人物工作区及两列子项增加 `min-height: 0` 与有界 overflow，让照片舞台在桌面窗口内收缩。
   - Post-fix evidence: `screenshots/03-people.png`；浏览器检查 `filmstripVisible: true`、`actionsVisible: true`。

2. Main-path interaction pass
   - Finding: [P1] 原型“页面地图”悬浮入口覆盖关键人物页“保存并开始”，阻断主流程点击。
   - Fix: 将评审入口移入顶栏左侧空白区，避开所有产品主操作。
   - Post-fix evidence: 完整主路径点击通过：`setup → people → analysis → ready → review → confirm → write → library-result`。

3. Final visual comparison
   - Earlier P1/P2 findings均已修复。
   - Source and implementation were compared together in `screenshots/design-qa-comparison.png`.
   - Browser console errors checked: none.

## Primary interactions tested

- Library 启动 → 配置 → 人物选择 → 分析 → 完成通知 → 复核 → 确认 → 写入 → Library 回显。
- 取消分析 → 保留部分完成结果 → 进入复核。
- 修改 AI 星级 → 来源转人工 → AI 原因清除 → 保护状态生效。
- 离开复核 → 放弃确认。
- 写入失败项重试 → 成功项保持、失败数更新。
- 设备不支持 → 仅阻止智能选图、返回 Library。

## Implementation checklist

- 生产接入真实任务数据时保持现有页面状态模型和来源转换规则。
- 按 P0-P3 验证门替换原型模拟的设备、模型、进度、原因和写入数据。
- 接入中英文 feature-local 文案并验证较长英文的换行与省略。

final result: passed
