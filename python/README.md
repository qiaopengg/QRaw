# RapidRAW Python 工具集

## 📌 总览

本目录包含 RapidRAW 的 **Python 离线工具**，用于训练、评测和数据处理。

**重要**：这些工具**不是运行时服务**，而是开发和优化工具。

---

## 📁 目录结构

```
python/
├── README.md                      # 本文件
├── style_transfer_training/       # 离线训练工具
│   ├── train_preset_predictor.py
│   ├── train_lut_model.py
│   └── export_onnx.py
├── style_transfer_eval/            # 离线评测工具
│   ├── benchmark.py
│   ├── metrics.py
│   └── regression_test.py
├── style_transfer_tools/           # 数据处理工具
│   ├── dataset_builder.py
│   ├── feature_extractor.py
│   └── visualization.py
└── style_transfer_service/         # ⚠️ 已废弃
    └── README_DEPRECATED.md
```

---

## 🎯 工具定位

### 1. 训练工具（`style_transfer_training/`）

**用途**：离线训练模型

**功能**：

- Preset Predictor 训练
- LUT 模型训练
- ONNX 导出

**何时使用**：

- Phase 2：建立学习型映射
- Phase 3：优化模型效果

### 2. 评测工具（`style_transfer_eval/`）

**用途**：离线评测和测试

**功能**：

- Benchmark 测试
- 回归测试
- 性能测试

**何时使用**：

- Phase 1：建立评测基线
- 每次代码变更后：回归测试
- 发布前：完整 Benchmark

### 3. 数据处理工具（`style_transfer_tools/`）

**用途**：数据准备和分析

**功能**：

- 数据集构建
- 特征提取
- 可视化

**何时使用**：

- Phase 1：准备测试集
- Phase 2：准备训练数据
- 调试时：可视化分析

### 4. ⚠️ 已废弃服务（`style_transfer_service/`）

**状态**：已废弃，不再使用

**原因**：运行时已完全迁移到 Rust

**详情**：查看 `style_transfer_service/README_DEPRECATED.md`

---

## 🚀 快速开始

### 安装依赖

每个工具目录都有独立的 `requirements.txt`：

```bash
# 训练工具
cd style_transfer_training
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt

# 评测工具
cd style_transfer_eval
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt

# 数据处理工具
cd style_transfer_tools
python -m venv .venv
source .venv/bin/activate
pip install -r requirements.txt
```

### 使用示例

#### 1. 构建数据集

```bash
cd style_transfer_tools
python dataset_builder.py \
  --input /path/to/images \
  --output /path/to/dataset
```

#### 2. 提取特征

```bash
cd style_transfer_tools
python feature_extractor.py \
  --dataset /path/to/dataset \
  --output /path/to/features
```

#### 3. 运行 Benchmark

```bash
cd style_transfer_eval
python benchmark.py \
  --test-set /path/to/test-set \
  --output results/benchmark_report.json
```

#### 4. 训练模型

```bash
cd style_transfer_training
python train_preset_predictor.py \
  --dataset /path/to/dataset \
  --output models/preset_predictor.onnx
```

---

## 📊 工作流程

### Phase 1：规范输入 + 局部止血

```
1. 准备测试集
   └─> style_transfer_tools/dataset_builder.py

2. 建立评测基线
   └─> style_transfer_eval/benchmark.py

3. 开发新功能（Rust）
   └─> src-tauri/src/style_transfer.rs

4. 运行回归测试
   └─> style_transfer_eval/regression_test.py
```

### Phase 2：学习型映射核心

```
1. 准备训练数据
   └─> style_transfer_tools/dataset_builder.py
   └─> style_transfer_tools/feature_extractor.py

2. 训练模型
   └─> style_transfer_training/train_preset_predictor.py
   └─> style_transfer_training/train_lut_model.py

3. 导出 ONNX
   └─> style_transfer_training/export_onnx.py

4. 集成到 Rust
   └─> src-tauri/src/style_transfer.rs

5. 评测效果
   └─> style_transfer_eval/benchmark.py
```

### Phase 3：学习型 Preset 与产品闭环

```
1. 收集用户行为数据
   └─> style_transfer_tools/user_behavior_collector.py

2. 更新训练数据
   └─> style_transfer_tools/dataset_builder.py

3. 重新训练模型
   └─> style_transfer_training/train_preset_predictor.py

4. 评测和部署
   └─> style_transfer_eval/benchmark.py
```

---

## 🔗 与 Rust 运行时的关系

### Python 工具的职责

- ✅ **离线训练**：训练模型
- ✅ **离线评测**：测试效果
- ✅ **数据处理**：准备数据
- ✅ **可视化**：分析结果

### Rust 运行时的职责

- ✅ **实时推理**：加载 ONNX 模型
- ✅ **风格迁移**：执行主流程
- ✅ **参数应用**：应用到图像
- ✅ **用户交互**：响应用户操作

### 数据流

```
Python 训练工具
  ↓ 训练模型
  ↓ 导出 ONNX
  ↓
~/.qraw/models/
  ↓ 模型文件
  ↓
Rust 运行时
  ↓ 加载模型
  ↓ 实时推理
  ↓
用户界面
```

---

## 📚 相关文档

- **V4 架构文档**：`docs/rapid_raw_分析式风格迁移技术架构_v_4.md`
- **深度评估报告**：`DEEP_EVALUATION_REPORT.md`
- **实施状态**：`PHASE_IMPLEMENTATION_STATUS.md`
- **训练工具文档**：`style_transfer_training/README.md`
- **评测工具文档**：`style_transfer_eval/README.md`
- **数据处理工具文档**：`style_transfer_tools/README.md`

---

## ⚠️ 重要提醒

### 不要使用 `style_transfer_service/`

**该目录已废弃**，不再作为运行时服务使用。

如果您需要：

- **运行时风格迁移**：使用 Rust 实现（`src-tauri/src/style_transfer.rs`）
- **离线训练/评测**：使用新的 Python 工具（本目录）

详情查看：`style_transfer_service/README_DEPRECATED.md`

---

## 📝 开发状态

| 工具                  | 状态      | Phase   |
| --------------------- | --------- | ------- |
| 数据集构建            | ⏳ 待实现 | Phase 1 |
| 特征提取              | ⏳ 待实现 | Phase 1 |
| Benchmark 测试        | ⏳ 待实现 | Phase 1 |
| 回归测试              | ⏳ 待实现 | Phase 1 |
| Preset Predictor 训练 | ⏳ 待实现 | Phase 2 |
| LUT 模型训练          | ⏳ 待实现 | Phase 2 |
| ONNX 导出             | ⏳ 待实现 | Phase 2 |

---

**创建日期**：2026-04-27

**状态**：🚧 开发中

**维护者**：RapidRAW 团队
