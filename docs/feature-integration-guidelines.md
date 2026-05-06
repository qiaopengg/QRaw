# 二开 Feature 接入规范

## 目标

本规范用于约束所有二次开发功能的接入方式，确保：

- 二开功能独立维护
- 上游代码仅保留统一入口
- 后续同步 `CyberTimon/RapidRAW` 更新时尽量减少冲突
- 不因二开功能扩散而持续侵入上游代码

## 适用范围

适用于本仓库内所有新增或重构的二开功能，包括但不限于：

- 前端功能模块
- 前端快捷键与设置项
- Tauri command
- Rust 业务逻辑模块

## 强制原则

1. 严禁直接把二开功能实现散落写入上游业务文件。
2. 上游代码只允许保留统一入口、挂载点、透传层和必要注册层。
3. 二开功能的真实实现必须集中在独立 feature 目录内维护。
4. 抽离或重构二开功能时，不得顺手修改原有功能逻辑、交互行为或 UI 样式。
5. 新增二开功能时，优先复用已有统一 feature 架构，不得重新引入点状侵入。

## 目录规范

### 前端

所有前端二开功能必须放在：

```text
src/features/<feature-name>/
```

例如：

```text
src/features/focus-areas/
```

推荐结构：

```text
src/features/<feature-name>/
  contracts.ts
  constants.ts
  feature.tsx
  index.ts
  use<FeatureName>.tsx
  <FeatureEntry>.tsx
  <FeatureOverlay>.tsx
```

### Rust / Tauri

所有 Rust 二开功能必须放在：

```text
src-tauri/src/features/<feature-name>/
```

例如：

```text
src-tauri/src/features/focus_areas/
```

## 上游允许保留的内容

上游代码中只允许保留以下内容：

- 统一 feature 注册入口
- 通用插槽定义
- 通用类型定义
- 通用快捷键聚合入口
- Tauri 统一 command 注册入口

允许存在的典型文件包括：

- `src/features/appFeatures.ts`
- `src/features/contracts.ts`
- `src/features/keybindDefinitions.ts`
- `src/utils/keybindContracts.ts`
- `src-tauri/src/features/mod.rs`

这些文件的职责必须保持为“统一入口”或“通用契约”，不能再次长出具体业务实现。

## 明确禁止的做法

以下做法一律禁止：

- 在 `src/components/**` 中直接实现二开业务逻辑
- 在 `src/utils/**` 中直接写死某个二开功能的具体实现
- 在 `src/hooks/**` 中直接耦合某个二开 feature 的内部细节，除非该 hook 本身属于该 feature 目录
- 在 `src-tauri/src/lib.rs` 中直接实现二开业务逻辑
- 为了接入二开功能而修改上游现有逻辑分支、渲染细节或样式定义
- 将多个二开功能混杂在同一个 feature 目录中维护

## 前端接入规则

### 1. 功能状态与逻辑

功能状态、数据获取、业务计算、渲染逻辑必须放在对应 feature 目录内。

例如：

- hook 放在 `src/features/<feature-name>/use*.tsx`
- 入口组件放在 `src/features/<feature-name>/*Entry.tsx`
- overlay 或独立 UI 放在 feature 目录内部

### 2. 上游挂载方式

上游组件不得直接引用具体 feature 实现，而应通过统一 feature 注册层接入。

当前统一接入方式：

- `App` 通过 `src/features/appFeatures.ts` 获取已注册 feature
- `Editor` 系列组件通过通用 `editorFeatureSlots` 接收 feature UI
- 公共快捷键通过统一定义聚合入口接入

### 3. 快捷键

所有二开功能快捷键定义必须先声明在 feature 目录，再统一聚合到：

```text
src/features/keybindDefinitions.ts
```

禁止在以下位置直接导入某个具体 feature 的快捷键定义：

- `src/utils/keyboardUtils.ts`
- `src/components/panel/SettingsPanel.tsx`
- `src/hooks/useKeyboardShortcuts.tsx`

这些公共层只能依赖通用契约和统一聚合入口。

## Rust / Tauri 接入规则

### 1. 业务实现

具体功能实现必须放在：

```text
src-tauri/src/features/<feature-name>/
```

不得把二开业务逻辑直接写回：

- `src-tauri/src/lib.rs`
- 其他上游核心模块

### 2. command 暴露方式

`lib.rs` 中只允许保留 command 注册入口，不允许出现 feature 的业务实现。

推荐模式：

1. feature 子模块实现真实逻辑
2. `src-tauri/src/features/mod.rs` 提供统一导出或统一包装入口
3. `lib.rs` 只负责注册该统一入口

## 新增二开功能的标准流程

新增功能时必须按以下顺序进行：

1. 在 `src/features/<feature-name>/` 创建前端目录
2. 在 `src-tauri/src/features/<feature-name>/` 创建 Rust 目录
3. 在 feature 目录内部实现全部状态、逻辑、组件和常量
4. 在统一 feature 注册层接入前端入口
5. 在统一快捷键聚合层接入快捷键定义
6. 在 Rust `features/mod.rs` 接入统一导出
7. 在 `lib.rs` 中仅注册统一入口
8. 完成后全局扫描，确认上游目录不存在该 feature 的实现散落

## 重构和审查要求

每次新增或调整二开功能后，至少检查以下内容：

1. `src/components/**` 是否出现该 feature 的实现细节
2. `src/utils/**` 是否直接依赖具体 feature
3. `src/hooks/**` 是否出现本应留在 feature 目录的逻辑
4. `src-tauri/src/lib.rs` 是否只保留入口而没有业务实现
5. 全局搜索该 feature 关键字时，命中是否主要集中在 feature 目录和统一入口层

## 对焦区域标注的现状要求

`对焦区域标注` 已经按本规范完成独立维护。后续如继续修改该功能，必须遵守以下约束：

- 只在 `src/features/focus-areas/` 内修改前端主体
- 只在 `src-tauri/src/features/focus_areas/` 内修改 Rust 主体
- 若需新增接入点，优先扩展统一 feature 架构，不得重新把逻辑写回上游组件

## 一句话原则

本项目所有二开功能都必须是“外挂式 feature”，而不是“侵入式 patch”。
