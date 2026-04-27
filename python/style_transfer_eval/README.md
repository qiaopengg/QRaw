# 风格迁移离线评测工具

## 📌 定位

本目录包含风格迁移的**离线评测工具**，用于测试和验证模型效果。

**不是运行时服务**，而是质量保证工具。

---

## 🎯 功能

### 1. Benchmark 测试

在标准测试集上评测风格迁移效果。

**评测指标**：

- 风格接近度
- 肤色误差
- 高光溢出风险
- 饱和度过冲风险
- 局部区域一致性
- 参数可用率

### 2. 回归测试

确保新版本不会降低已有场景的效果。

**测试场景**：

- 人像
- 风光
- 城市夜景
- 室内低照度
- 高动态范围

### 3. 性能测试

测试运行时性能。

**性能指标**：

- 分析时间
- 内存占用
- GPU 占用
- 模型加载时间

---

## 🚀 快速开始

### 安装依赖

```bash
cd python/style_transfer_eval
python -m venv .venv
source .venv/bin/activate  # Windows: .venv\Scripts\activate
pip install -r requirements.txt
```

### 运行 Benchmark

```bash
python benchmark.py \
  --test-set /path/to/test-set \
  --output results/benchmark_report.json
```

### 运行回归测试

```bash
python regression_test.py \
  --baseline results/baseline.json \
  --current results/current.json \
  --output results/regression_report.json
```

### 运行性能测试

```bash
python performance_test.py \
  --test-set /path/to/test-set \
  --output results/performance_report.json
```

---

## 📁 目录结构

```
style_transfer_eval/
├── README.md                    # 本文件
├── requirements.txt             # Python 依赖
├── benchmark.py                 # Benchmark 测试
├── regression_test.py           # 回归测试
├── performance_test.py          # 性能测试
├── metrics.py                   # 评测指标
├── test_sets/                   # 测试集
│   ├── portrait/                # 人像测试集
│   ├── landscape/               # 风光测试集
│   ├── night/                   # 夜景测试集
│   └── indoor/                  # 室内测试集
└── results/                     # 评测结果
    ├── baseline.json            # 基线结果
    ├── current.json             # 当前结果
    └── reports/                 # 评测报告
```

---

## 📊 测试集格式

### Benchmark 测试集

```
test_sets/portrait/
├── sample_001/
│   ├── reference.jpg            # 参考图
│   ├── current.jpg              # 当前图
│   ├── ground_truth.jpg         # 真值（可选）
│   └── metadata.json            # 元数据
├── sample_002/
└── ...
```

**metadata.json 示例**：

```json
{
  "scene_type": "portrait",
  "difficulty": "medium",
  "expected_metrics": {
    "style_similarity": 0.85,
    "skin_error": 0.05,
    "highlight_risk": 0.02
  }
}
```

---

## 📈 评测指标

### 1. 风格接近度

**定义**：生成结果与参考图的风格相似度

**计算方法**：

- 全局特征相似度
- 局部区域相似度
- 色彩分布相似度

**目标**：> 0.80

### 2. 肤色误差

**定义**：人像照片中肤色的偏差

**计算方法**：

- 肤色区域检测
- 色相、饱和度、亮度误差
- 与参考图肤色对比

**目标**：< 0.10

### 3. 高光溢出风险

**定义**：高光区域过曝的风险

**计算方法**：

- 高光区域占比
- 高光细节保留度
- 动态范围评估

**目标**：< 0.05

### 4. 饱和度过冲风险

**定义**：颜色过饱和的风险

**计算方法**：

- 饱和度分布
- 过饱和像素占比
- 色彩自然度

**目标**：< 0.05

### 5. 局部区域一致性

**定义**：不同区域风格的一致性

**计算方法**：

- 天空、人物、背景区域分析
- 区域间风格差异
- 区域内风格统一度

**目标**：> 0.75

### 6. 参数可用率

**定义**：生成的参数建议的可用性

**计算方法**：

- 用户应用率
- 用户调整幅度
- 用户撤销率

**目标**：> 0.60

---

## 📊 评测报告

### Benchmark 报告

```json
{
  "test_set": "portrait",
  "total_samples": 100,
  "metrics": {
    "style_similarity": {
      "mean": 0.82,
      "std": 0.08,
      "min": 0.65,
      "max": 0.95
    },
    "skin_error": {
      "mean": 0.07,
      "std": 0.03,
      "min": 0.02,
      "max": 0.15
    },
    "highlight_risk": {
      "mean": 0.03,
      "std": 0.02,
      "min": 0.0,
      "max": 0.08
    }
  },
  "pass_rate": 0.85,
  "failed_samples": ["sample_023", "sample_047"]
}
```

### 回归测试报告

```json
{
  "baseline_version": "v1.0.0",
  "current_version": "v1.1.0",
  "regression_detected": false,
  "improvements": [
    {
      "metric": "style_similarity",
      "baseline": 0.78,
      "current": 0.82,
      "improvement": "+5.1%"
    }
  ],
  "regressions": [],
  "overall_status": "PASS"
}
```

---

## 🔧 配置

### Benchmark 配置

`configs/benchmark.yaml`:

```yaml
test_sets:
  - name: portrait
    path: test_sets/portrait
    weight: 1.0
  - name: landscape
    path: test_sets/landscape
    weight: 1.0
  - name: night
    path: test_sets/night
    weight: 0.8

metrics:
  - style_similarity
  - skin_error
  - highlight_risk
  - saturation_risk
  - local_consistency
  - parameter_usability

thresholds:
  style_similarity: 0.80
  skin_error: 0.10
  highlight_risk: 0.05
  saturation_risk: 0.05
  local_consistency: 0.75
  parameter_usability: 0.60
```

---

## 🔗 与开发流程集成

### 1. 建立基线

```bash
# 在当前版本上运行 benchmark
python benchmark.py --test-set test_sets/ --output results/baseline.json
```

### 2. 开发新功能

```bash
# 修改代码...
```

### 3. 运行回归测试

```bash
# 在新版本上运行 benchmark
python benchmark.py --test-set test_sets/ --output results/current.json

# 对比基线和当前版本
python regression_test.py \
  --baseline results/baseline.json \
  --current results/current.json \
  --output results/regression_report.json
```

### 4. 查看报告

```bash
cat results/regression_report.json
```

---

## 📚 相关文档

- **V4 架构文档**：`docs/rapid_raw_分析式风格迁移技术架构_v_4.md`
- **训练工具**：`python/style_transfer_training/`
- **数据处理工具**：`python/style_transfer_tools/`

---

## 🎯 Phase 1 目标

根据 V4 文档，Phase 1 的目标是：

- ✅ 建立最小人工评测集
- ✅ 建立 Benchmark 测试
- ✅ 建立回归测试机制

---

## 📝 开发状态

| 功能           | 状态      | 说明    |
| -------------- | --------- | ------- |
| Benchmark 测试 | ⏳ 待实现 | Phase 1 |
| 回归测试       | ⏳ 待实现 | Phase 1 |
| 性能测试       | ⏳ 待实现 | Phase 1 |
| 测试集构建     | ⏳ 待实现 | Phase 1 |

---

**创建日期**：2026-04-27

**状态**：🚧 开发中（Phase 1）
