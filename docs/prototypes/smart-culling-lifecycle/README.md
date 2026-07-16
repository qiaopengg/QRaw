# QRaw 智能选图全生命周期设计原型

这是依据 `docs/smart-culling-v2-requirements.md` v3.0 重新生成的独立 React 设计原型，
用于验证和指导 QRaw macOS/Windows 客户端的完整智能选图流程，不是生产功能实现。

## 本地运行

```bash
npm install
npm run dev
```

默认入口为 `http://127.0.0.1:5173/?screen=library`。页面顶部的“页面地图”可以直接打开全部十个
主页面；取消分析、部分完成、人工保护、放弃确认和失败重试通过页面内操作进入。

## 主页面路由

- `?screen=library`
- `?screen=setup`
- `?screen=people`
- `?screen=analysis`
- `?screen=ready`
- `?screen=review`
- `?screen=confirm`
- `?screen=write`
- `?screen=library-result`
- `?screen=unsupported`

## 原型边界

- 图片、统计、进度、设备信息和写入结果是用于交互验收的真实感模拟数据。
- “演示：完成分析”和“页面地图”是原型评审工具，不进入生产客户端。
- 生产开发必须继续遵守需求文档的 P0-P3 验证门，不能把可操作原型当成模型、性能或数据安全已完成。
