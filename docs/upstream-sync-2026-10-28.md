# 2026-10-28 上游同步与功能隔离记录

## 同步范围

- 上游：`CyberTimon/RapidRAW` 的 `main`。
- 本次同步目标：`bd509d62fde32846545f93872b20747c705c1b7b`，提交时间为 2026-10-27 21:58:49 +0100；应用版本为 `1.6.7`。
- 合并所在分支：`feat-ai-chat`。
- 合并前恢复分支：`codex/pre-upstream-sync-20261028`。
- 合并前本地独有 47 个提交，上游待合入 529 个提交。采用保留双方历史的 merge，不重写本地历史。
- 本次只完成本地同步，未推送远程。

## 项目职责与隔离方式

上游负责图库、RAW 解码与编辑、GPU 渲染、导出、原生 Auto Cull、Immich 等通用照片工作流。本地扩展包括对焦区域标注、智能选图及其复核与写回流程，继续在 `src/features/` 和 `src-tauri/src/features/` 中独立维护。

遵循 `docs/feature-integration-guidelines.md`：宿主只提供通用插槽、快捷键契约、命令注册和扩展数据透传；具体二开业务不进入上游组件、hook 或图像处理模块。

上游 Auto Cull 与本地 Smart Culling 保持各自入口、状态和事件：

| 项目 | 上游原生选图 | 本地智能选图 |
| --- | --- | --- |
| 入口 | 图库右键 → Productivity → Auto Cull | 图库 Smart Culling 按钮与独立复核页 |
| 后端命令 | `cull_images` | `smart_culling_command` |
| 事件 | `culling-start/progress/complete/error` | `smart-culling://event` |
| 数据 | 上游评分、旗标及编辑元数据 | 通过通用 `featureData` 保存扩展记录 |

星级和颜色仍是双方有意共用的照片元数据；隔离不表示这些公共字段互相不可见。本地既有人工保护、锁定和写回冲突检查继续生效；本次新增回归验证双方保存时保留彼此专有字段。

## 合并与兼容修改

### 恢复并保留完整上游行为

- 恢复此前本地移除的 `src-tauri/src/culling.rs` 与 `src/components/modals/CullingModal.tsx`，内容与本次上游目标一致。
- 恢复原生选图的菜单、弹窗、状态、事件、快捷键处理及 Tauri command 注册。
- 保留上游新加入的 Pick/Reject、Immich、元数据批量更新、新编辑器布局与渲染接口。
- `package.json`、`package-lock.json`、`Cargo.toml`、`Cargo.lock` 使用上游版本；Rust 按上游要求使用 `1.98` 工具链，没有额外增加二开依赖。

### 通用宿主接入

- 前端 App、Editor、Library 及相关视图仅接入现有 feature slots；未注册扩展时使用原来的默认分支。
- `src/utils/keybindContracts.ts` 同步上游 `macDefaultCombo`，快捷键通过统一聚合入口接入。
- `src/hooks/useTauriListeners.ts` 在上游缓冲批处理机制中透传 `featureData`；收到空值时清除旧扩展数据。
- `src-tauri/src/lib.rs` 仅保留 feature 模块与统一命令注册。
- `image_processing.rs`、`file_management.rs` 保留通用扩展字段及列表、事件透传。
- `exif_processing.rs` 的默认侧车清理增加一项通用判定：仍有 `featureData` 时保留文件，防止上游保存操作删除二开记录。

后端宿主相对上游仅以上四个 Rust 文件存在 19 行通用接入追加；具体适配与回归测试均放在 feature 目录。

### 智能选图内部适配

| 文件 | 修改原因 |
| --- | --- |
| `infrastructure/render_input.rs` | 适配上游 `AsShot`、`RenderOptions` 和渲染缓存参数；从同一份源字节读取拍摄白平衡。 |
| `infrastructure/decode_snapshot.rs` | 基础解码继续使用隔离快照，像素准备使用真实源路径复用上游效果缓存。 |
| `infrastructure/decode_snapshot_tests.rs` | 验证非默认负片效果的像素输出与上游处理一致。 |
| `infrastructure/catalog.rs` | 在可能隔离损坏侧车的上游 EXIF 读取之前严格校验，防止损坏元数据被误当成无保护；回归确认原始损坏字节不变。 |
| `infrastructure/persistence/asset_tests.rs` | 验证 AI 写回、人工同步、锁定变化保留 Pick/Reject，普通上游保存保留扩展数据。 |
| 前端 `sourceEligibility.ts` | 复用上游 Immich 标识判断，集中定义本地输入范围。 |
| 前端 `SmartCullingEntry.tsx`、`SmartCullingLockControl.tsx`、`SmartCullingReviewPage.tsx`、`i18n.ts` | 排除云端占位和 Immich 相册写入，不将相册显示名称当作目录；本地相册中已有实体图仍可人工同步和锁定。 |
| 前端 `tests/taskSelection.test.mjs` | 新增 5 项本地目录、云占位、相册和虚拟副本边界测试。 |

本次未调整评分算法、模型权重、阈值、校准参数或发布门禁。复用现有 feature 接入与上游解码工具；新增路径判断独立成小模块，未向大型宿主文件添加业务逻辑。

## 验证结果

| 检查 | 结果 |
| --- | --- |
| `cargo +1.98 test --manifest-path src-tauri/Cargo.toml --lib --locked --offline` | 331 通过，0 失败，14 忽略；忽略项为既有需显式启用的硬件/模型实验。合并前为 328 通过、14 忽略。 |
| `node --test src/features/smart-culling/tests/*.test.mjs` | 24 通过，0 失败；合并前为 19 通过。 |
| `python3 -m unittest discover -s src-tauri/src/features/smart_culling/model_assets -p test_release_inventory.py -v` | 6 通过。 |
| `npm run build` | 通过，存在上游包体积提示。 |
| `RUSTUP_TOOLCHAIN=1.98 npm run tauri -- build --debug --bundles app --no-sign --ci -- --locked --offline` | macOS debug 应用构建及打包通过；保留现有 Rust 警告，未进行签名。 |
| `release_inventory.py` 检查源码资源及 `.app/Contents/Resources` | 模型完整性与校准资源隔离通过；不代表发布验收通过。 |
| `rustfmt +1.98 --edition 2024 --check`（本次改动的 5 个 feature Rust 文件） | 通过。 |
| `git diff --check`、`git diff --cached --check upstream/main` | 本次适配及相对上游的本地差异检查通过。 |
| `git diff --cached --check`（整个合并相对本地旧版本） | 上游原样文件有 3 处尾随空格：`bench/replay.js` 1 处、DirectML 许可证 2 处；未改写上游内容。 |
| `npm run typecheck` | 未通过：65 个错误。与纯上游目标的错误路径及错误码多重集一致，二开目录无错误。 |
| `npm run lint -- --format json` | 未通过：849 错误、44 警告。纯上游为 849 错误、45 警告；错误签名无新增。 |

TypeScript 与 ESLint 的既有上游错误未在本次合并中扩范围修复，因此不能将全量静态检查表述为通过。TypeScript 中 LibraryGrid 的同一类型错误描述会带出新增插槽参数名，但错误路径、错误码及根本类型不兼容与纯上游一致。

### macOS 界面冒烟验证

使用临时目录中的两张仓库 splash JPEG 样例完成以下检查：

1. 应用启动，显示版本 `1.6.7`。
2. 图库正常展示缩略图，编辑器正常渲染图像。
3. 本地 Smart Culling 完成风景分析并进入复核页，0 扫描失败；该样例被标记为需人工复核，未执行确认写回。
4. 上游 Auto Cull 对两张样例完成计算，正常显示结果弹窗。
5. 测试后移除临时目录的图库引用并退出测试应用。

最后一次打包包含随后完成的云相册边界保护；该保护通过新增 5 项自动化回归验证。

## 验证边界与复现资料

- 本次验证覆盖本机 macOS 的测试、构建和上述基础交互，不等于 Windows、Linux、Android 或真实 Immich 服务验收。
- 未执行相机联机、全部 RAW 机型、真实人像校准、批量性能、签名发布与安装器验收。14 项忽略测试和现有发布门禁没有被取消。
- 对焦区域标注代码保持原样并参与构建；真实相机对焦元数据覆盖仍需对应照片样本验证。
- 本次可运行构建：`src-tauri/target/debug/bundle/macos/RapidRAW.app`，为未签名 debug 包。
- 后端、构建及资源检查日志：`/tmp/qraw-upstream-sync-ocxcw3`。
- 前端与纯上游对照日志：`/var/folders/mf/32brj6rn5qdgwddgl88f4brw0000gn/T/qraw-frontend-merge-knqwqud7`。
- 临时日志可能由系统清理；本文件记录可复现命令与结论。恢复分支保存合并前代码，后续同步应继续检查宿主差异及上述数据互保回归。
