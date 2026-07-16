import coupleKiss from "./assets/photos/couple-kiss.png";
import ringExchange from "./assets/photos/ring-exchange.png";
import churchWide from "./assets/photos/church-wide.png";
import bridePortrait from "./assets/photos/bride-portrait.png";
import confettiExit from "./assets/photos/confetti-exit.png";
import fatherBride from "./assets/photos/father-bride.png";

export const lifecyclePages = [
  { id: "library", group: "开始", label: "01  Library 入口", summary: "从当前文件夹启动智能选图" },
  { id: "setup", group: "配置", label: "02  任务配置", summary: "设备预检、模式与范围摘要" },
  { id: "people", group: "配置", label: "03  关键人物", summary: "点选人物并设置优先级" },
  { id: "analysis", group: "分析", label: "04  后台分析", summary: "只读浏览、真实进度与取消" },
  { id: "ready", group: "分析", label: "05  完成通知", summary: "待复核入口与部分完成状态" },
  { id: "review", group: "复核", label: "06  结果复核", summary: "故事组、人工保护与失败项" },
  { id: "confirm", group: "确认", label: "07  确认应用", summary: "写入前变更摘要与冲突检查" },
  { id: "write", group: "确认", label: "08  写入结果", summary: "部分失败、重试与保留成功项" },
  { id: "library-result", group: "完成", label: "09  Library 回显", summary: "星级、标签、来源与原因" },
  { id: "unsupported", group: "分支", label: "10  设备不支持", summary: "阻止启动并解释原因" },
];

export const folders = [
  { id: "prep", label: "00 准备", count: 236 },
  { id: "first-look", label: "01 初见", count: 312 },
  { id: "ceremony", label: "02 仪式", count: 684 },
  { id: "portrait", label: "03 合影", count: 412 },
  { id: "toast", label: "04 敬酒", count: 786 },
  { id: "dance", label: "05 舞会", count: 1286 },
  { id: "exit", label: "06 送客", count: 96 },
];

export const shootModes = [
  { id: "auto", label: "自动 / 混合场景", description: "自动判断不同场景并组织结果" },
  { id: "landscape", label: "风光", description: "构图、光线与细节优先" },
  { id: "portrait", label: "人像", description: "表情、眼睛与主体清晰度优先" },
  { id: "environment", label: "环境人像", description: "人物与环境叙事平衡" },
  { id: "group", label: "群像", description: "多人表情与遮挡优先" },
  { id: "documentary", label: "街拍 / 纪实", description: "事件瞬间与故事连续性优先" },
  { id: "wildlife", label: "动物 / 鸟类", description: "主体动作与眼部清晰度优先" },
  { id: "architecture", label: "建筑 / 空间", description: "线条、透视与完整性优先" },
  { id: "product", label: "产品 / 静物", description: "质感、细节与背景干净度优先" },
  { id: "astro", label: "星空 / 银河", description: "星点、噪点与堆栈保护优先" },
];

export const photos = [
  {
    id: "DSC_4120.ARW",
    src: coupleKiss,
    rating: 2,
    label: "reject",
    source: "AI",
    reason: "主体略偏离焦点，同组中表情较弱",
    selected: false,
    confidence: 86,
  },
  {
    id: "DSC_4121.ARW",
    src: coupleKiss,
    rating: 3,
    label: "review",
    source: "AI",
    reason: "主体清晰，但构图与同组最佳接近",
    selected: true,
    confidence: 74,
  },
  {
    id: "DSC_4122.ARW",
    src: coupleKiss,
    rating: 5,
    label: "pick",
    source: "AI",
    reason: "构图居中，表情自然亲密；光线柔和，背景干净",
    selected: true,
    confidence: 92,
  },
  {
    id: "DSC_4123.ARW",
    src: ringExchange,
    rating: 4,
    label: "pick",
    source: "AI",
    reason: "戒指动作完整，手部细节清晰",
    selected: true,
    confidence: 88,
  },
  {
    id: "DSC_4124.ARW",
    src: bridePortrait,
    rating: 2,
    label: "reject",
    source: "AI",
    reason: "同组存在更自然的表情与视线",
    selected: false,
    confidence: 81,
  },
  {
    id: "DSC_4187.ARW",
    src: ringExchange,
    rating: 4,
    label: "pick",
    source: "人工",
    reason: "",
    selected: true,
    confidence: null,
  },
  {
    id: "DSC_4201.ARW",
    src: confettiExit,
    rating: 4,
    label: "pick",
    source: "AI",
    reason: "关键瞬间完整，人物关系清晰",
    selected: true,
    confidence: 89,
  },
  {
    id: "DSC_4212.ARW",
    src: fatherBride,
    rating: 3,
    label: "review",
    source: "AI",
    reason: "情绪价值较高，但存在轻微运动模糊",
    selected: true,
    confidence: 67,
  },
];

export const hdrPhotos = [
  { id: "HDR -1 EV", src: churchWide },
  { id: "HDR 0 EV", src: churchWide },
  { id: "HDR +1 EV", src: churchWide },
];

export const labelMeta = {
  pick: { label: "精选", color: "green" },
  review: { label: "待确认", color: "yellow" },
  reject: { label: "淘汰建议", color: "red" },
};
