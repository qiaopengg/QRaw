# eDifFIQA(T) 人脸质量观测模型

状态：2026-09-08 已接入 macOS Debug 的人像分析与复核证据；**仅观测，不参与清晰度门禁或星级计算**。Release 和其他平台不加载该模型。模型不可用时保留局部细节和光学观测，不能把失败转换成质量 0 分或 AI 0 星。

## 来源、署名与许可

- 模型：eDifFIQA(T)，来自 [OpenCV Zoo 官方模型目录](https://github.com/opencv/opencv_zoo/tree/47534e27c9851bb1128ccc0102f1145e27f23f98/models/face_image_quality_assessment_ediffiqa)。感谢 eDifFIQA 作者与 OpenCV Zoo 贡献者；原作者实现见 [LSIbabnikz/eDifFIQA](https://github.com/LSIbabnikz/eDifFIQA)。QRaw 没有重新训练或宣称拥有这些原始权重。
- 固定上游提交：`47534e27c9851bb1128ccc0102f1145e27f23f98`。
- 原始文件：`../ediffiqa_tiny_jun2024.onnx`，7,272,678 字节。
- 原始 SHA-256：`9426c899cc0f01665240cb7d9e7f98e18e24e456c178326c771a43da289bfc6a`。
- 运行文件：`../ediffiqa_tiny_jun2024_coreml_qraw.onnx`。
- 运行文件 SHA-256：`e25bc043793a0bcda0384f88eb8a04a01d4c6c826ec4a86dc076eba33b863557`。
- 上游目录标明 [CC-BY-4.0](https://github.com/opencv/opencv_zoo/blob/47534e27c9851bb1128ccc0102f1145e27f23f98/models/face_image_quality_assessment_ediffiqa/LICENSE)；保留该许可及其免责条款，模型按原样提供，不作准确性保证。QRaw 的使用不代表作者或 OpenCV 对产品背书。许可文本见 [Creative Commons Attribution 4.0](https://creativecommons.org/licenses/by/4.0/legalcode)。发布前仍需核查完整模型与数据权利链。

## 输入与输出契约

1. 复用现有 YuNet 五点及 `face_identity::align_face` 的 112×112 模板与相似变换。模板与 OpenCV 示例一致；本项目采用现有 SVD 拟合和 imageproc 双线性采样，不宣称与示例的 OpenCV LMEDS/插值逐像素相同。
2. RGB，NCHW float32 `[1,3,112,112]`，`pixel / 127.5 - 1`。不交换为 BGR，不作美颜或额外锐化。
3. ONNX 输入 `input`，固定 `batch_size=1`；输出 `output` 必须为有限的 `[1,1]` 数值。
4. 输出保留原始回归分，不裁切成概率，不使用官网示例数据分布作为本项目清晰度通过线。
5. 当前输入有效性限制：脸框完整在图内，最短边至少 32 原像素，检测达到项目已有可靠门槛，五点在脸框内且姿态几何可用；大侧脸暂不输出模型分。这些是观测输入保护，不是模糊标签。遮挡与对齐填边仍需专项实测。

## 等价图转换与复现

官方图在项目当前 Core ML MLProgram 路径中存在不受支持的独立 BatchNormalization/Abs 运算。复用项目已有 ONNX 图转换方式，仅作两项等价转换：

- 推理态 BatchNormalization 写成固定 `scale * x + offset`。
- `abs(x)^2` 写成 `x*x`。

其他卷积、回归头及精确 GELU 运算不变；不使用近似激活，不开放 ORT CPU EP 回退。Core ML 的 `CPUAndGPU` 是现有系统调度设置，**不等于声称每个运算都在 GPU 硬件上执行**。[Core ML 执行提供程序说明](https://onnxruntime.ai/docs/execution-providers/CoreML-ExecutionProvider.html)

在项目根目录运行：

```sh
python3 src-tauri/src/features/smart_culling/model_assets/ediffiqa/export_and_verify.py
```

脚本验证源/派生哈希，在临时目录重建图，对照原始 CPU 图的 36 组输入；模型缺失时可加 `--write` 写入已验证副本，绝不覆盖不同内容的文件。现有 Python `onnx`、`onnxruntime`、`numpy` 仅用于离线复现，没有新增应用依赖或运行时下载。

硬件检查需将 `ORT_DYLIB_PATH` 指向本项目 `src-tauri/resources/libonnxruntime.dylib`，运行：

```sh
cargo +1.96.1 test --manifest-path src-tauri/Cargo.toml features::smart_culling::portrait_quality::model::tests::ediffiqa_runs_with_the_project_gpu_runtime -- --ignored --nocapture
```

真实文件观测入口为忽略测试 `features::smart_culling::portrait_quality::calibration_export::export_portrait_quality_observations`。必须显式设置 `QRAW_PORTRAIT_QUALITY_IMAGE_DIR` 和尚不存在的绝对报告路径 `QRAW_PORTRAIT_QUALITY_OUTPUT`。它只读指定目录排序后的前 12 个 JPEG/PNG，以原检测框比较原图、σ=2 和 σ=6 的高斯模糊变体，另存 JSON 和原文件哈希，不改原图或表情标签。该离线入口注明 `decodedRgb8`，不能冒充应用内的渲染输入。硬件测试沿用现有隔离子进程退出方案规避 ORT 1.22 的日志析构问题，应用代码不执行该退出操作。

## 本机验证结果

- 官方 CPU 图与等价 CPU 图：36 组输入，最大绝对误差 `1.49011612e-7`，通过预设 `atol=1e-4, rtol=1e-5` 检查。
- 项目实际运行库的严格 Core ML 路径：加载、推理、4 组官方参考数值及对齐路径检查通过；没有 ORT CPU EP 回退。
- `/Users/qiaopeng/Desktop/测试/智能筛选` 的 12 个测试文件：25 张检测人脸、75 次变体质量推理，均有数值输出；对照同一对齐输入的官方 CPU 图，最大误差 `2.384185791015625e-7`，通过预设绝对误差 `<1e-3` 检查。
- σ=6 后，25/25 人脸的原尺度拉普拉斯、受控尺度拉普拉斯、Sobel 能量和 FIQA 分数均低于原图。原图观测耗时约 3–54 ms/张（不含原有人脸检测、模型冷启动和官方 CPU 对照；不是完整选图吞吐量）。
- 诊断报告：`/private/tmp/qraw-portrait-quality-probe-20260908.json`。临时文件可能被系统清理，可用上述只读入口重跑。

这些是工程和模糊敏感性验证，**没有清晰度人工真值、误弃率或未知比例验收**。前几轮表情标签不用于清晰度校准；噪声、锐化、美颜、光线、RAW/预览差异仍需独立评测。
