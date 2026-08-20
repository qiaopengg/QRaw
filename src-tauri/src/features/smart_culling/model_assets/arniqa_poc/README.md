# ARNIQA 隔离 POC（禁止生产打包）

本目录只用于验证 ARNIQA 的工程可行性，严格隔离于智能选图生产链路。本机生成的两个 ONNX 文件由
同目录 `.gitignore` 明确排除，不得提交或再分发，也没有加入 `src-tauri/resources`、Tauri
`bundle.resources`、生产模型加载器、`runner` 或评分代码。当前结论是：

- 技术 POC 通过 PyTorch/ONNX CPU 数值对齐；Python ONNX Runtime 1.19.2 完成了 Core ML EP
  严格无 CPU 回退推理，但 QRaw 内置 ONNX Runtime 1.22 的同一严格门禁未通过；
- Windows DirectML 尚未运行验证；
- 官方权重及训练数据没有给出足以确认商业再分发的明确条款；
- 因此这些资产**不得进入安装包、自动打星链路或发布制品**。

这不是“照片明确失焦”的分类器。即使未来通过法律和双平台门禁，ARNIQA 也只能作为光学质量和人物
区域质量的一项证据，不能单独触发“明确不清晰直接 1 星”。

## 1. 固定上游来源

审计和导出日期：2026-08-20。

- 官方仓库：<https://github.com/miccunifi/ARNIQA>
- 审计提交：`66d16eb0ff1e1655872d32c0c233614a3922aaad`
- 导出时实际执行的 `models/resnet.py` SHA-256：
  `07084106f7e096529fc584d755a5ab9f9ef94fdaf575a85053b9b39604140c49`；脚本不接受其他内容；
- 提交时间：`2026-06-18T05:10:10Z`
- 官方权重 Release：<https://github.com/miccunifi/ARNIQA/releases/tag/weights>
- Release ID：`134917776`
- 发布时间：`2023-12-22T15:10:08Z`

本机访问 `github.com:443` 超时，源码通过 GitHub 官方 `codeload.github.com` 通道取得，Release 元数据
和二进制通过 GitHub 官方 REST API 取得。没有使用镜像站或第三方权重。

| 下载对象 | 官方 Release 资产 ID | 字节数 | SHA-256 |
| --- | ---: | ---: | --- |
| `ARNIQA.pth` | `141995005` | `112155914` | `ad2022e59b1040d5bab24f9325c10d0215956a2061248a36c15edaec3e60fcd1` |
| `regressor_spaq.pth` | `141995193` | `19678` | `dbee93f9a8deb3c8357af0b7d4598c153b4a10075be34f9b69daa1aa04e778e3` |
| `regressor_koniq10k.pth` | `172184822` | `19560` | `af8f127aca38a8e1082e066b5ee93e533bf5f33aaf9c76b3c31526ef901919e1` |
| `regressor_kadid10k.pth` | `141995177` | `19778` | `4315bf471d52eb7d3e5de1e2ac8bb465f8eec10cd724103c72a178b9aaa4aa3f` |

浏览器下载 URL 的固定前缀为
`https://github.com/miccunifi/ARNIQA/releases/download/weights/`；本次实际使用的官方 API 端点是
`https://api.github.com/repos/miccunifi/ARNIQA/releases/assets/{asset_id}`，请求头为
`Accept: application/octet-stream`。原始 `.pth` 只在临时隔离目录中用于导出，没有复制进项目，避免在
权重再分发许可未明确前额外保存/传播 112 MB checkpoint。

源码归档的实际 URL 是
`https://codeload.github.com/miccunifi/ARNIQA/tar.gz/refs/heads/main`，本地文件为 `18821489`
字节，SHA-256 为 `584811101095cdbd3dc54df1e9069e36e8ced8ada19673b7fd828b6140b7c142`。

## 2. checkpoint 结构核对

`ARNIQA.pth` 是 `OrderedDict`，共 322 个张量：

- `model.*`：318 个状态张量，对应 torchvision ResNet-50 去除分类层后的主干；学习参数为
  23,508,032 个；
- `projector.*`：4 个状态张量、4,458,624 个参数；官方质量推理只使用 `encoder(img)` 返回的第一个
  主干特征 `f`，投影头输出 `g` 不进入质量回归；
- POC 导出时仅移除这个不参与质量输出的投影头，没有改写主干参数。

三个回归头都是官方发布的 TorchScript `TorchLinearRegression`：每个包含 `1 × 4096` FP32 权重和
一个 FP32 bias。质量分的官方缩放区间为：SPAQ `[1,100]`、KonIQ-10k `[1,100]`、KADID-10k
`[1,5]`；三者都是 MOS，缩放后分数越高越好。

## 3. POC 图契约

采用“一个共享 encoder + 一个可替换多头小图”，而不是导出三份完整 ResNet：

| 文件 | 输入 | 输出 | 字节数 | SHA-256 |
| --- | --- | --- | ---: | --- |
| 本机 `arniqa_encoder_224_poc.onnx` | `normalized_rgb: [1,3,224,224]` | `features: [1,2048]` | `93956612` | `a942e6aff3194d1111df41ee6513d471871f696c5d9e0df8360a84597d574dc5` |
| 本机 `arniqa_three_heads_poc.onnx` | `combined_embedding: [1,4096]` | `raw_scores/scaled_scores: [1,3]`，列顺序为 SPAQ、KonIQ-10k、KADID-10k | `49611` | `2a79928654c4b38c0375dbed596d393fc8d3d4b7ec9e7edb6097f0c0a192d441` |

encoder 只包含 `Conv`、`Relu`、`MaxPool`、`Add`、`GlobalAveragePool` 和 `Flatten`；小头只包含
`Gemm`、`Sub`、`Div`。两图均为固定 shape、ONNX opset 17。

应用侧必须执行且不能悄悄改变的官方契约：

1. 从原图生成全尺度图和宽高各减半的半尺度图；
2. 每个尺度取中心与四角共 5 个 `224 × 224` crop；
3. RGB 转 `[0,1]` 后按 ImageNet mean `(0.485, 0.456, 0.406)`、std
   `(0.229, 0.224, 0.225)` 归一化；
4. 每个 crop 调用共享 encoder；对 2048 维原始特征做 L2 归一化；
5. 拼接同位置的全尺度和半尺度 embedding 后调用回归头；
6. 对 5 个 crop 的同一回归头分数取平均。

L2 归一化和两个 embedding 的拼接有意保留在应用侧。原因是本机 ONNX Runtime 1.19.2 的 Core ML
EP 无法在严格模式完整接管 `ReduceL2/Expand` 和二维 `Concat`；移出这两个低成本操作后，两张图都能
在 `session.disable_cpu_ep_fallback=1` 下完成会话创建和真实推理。该拆分与官方 PyTorch 输出的数值
对齐已经验证。

## 4. 数值与执行提供程序验证

环境：macOS arm64，Python 3.9.6，PyTorch 2.8.0，torchvision 0.23.0，ONNX 1.17.0，
Python ONNX Runtime 1.19.2。输入为种子 `20260820` 产生的固定 FP32 张量。

| 对照 | 最大绝对误差 |
| --- | ---: |
| 精简 PyTorch encoder vs 官方 PyTorch embedding | `0.0` |
| 合并三头 PyTorch raw score vs 官方三个 TorchScript 头 | `3.814697265625e-06` |
| 合并三头 PyTorch scaled score vs 官方缩放 | `5.960464477539063e-08` |
| ONNX CPU embedding vs 精简 PyTorch embedding | `9.685754776000977e-08` |
| ONNX CPU raw score vs 精简 PyTorch | `7.62939453125e-06` |
| ONNX CPU scaled score vs 精简 PyTorch | `1.4901161193847656e-07` |
| 官方 `assets/01.png` 完整双尺度五 crop：ONNX CPU vs 官方 PyTorch scaled score | `2.384185791015625e-07` |

Python Core ML EP 使用 `MLProgram`、`CPUAndGPU`、`RequireStaticInputShapes=1`，并设置
`session.disable_cpu_ep_fallback=1`：

- encoder 和三头图都成功创建会话并执行真实推理；如果任何节点分配给默认 CPU EP，会话会直接失败；
- 单次观测的 encoder 会话创建约 `0.8041s`、推理约 `0.0023s`，小头约 `0.0136s/0.0002s`；这些只是
  POC 机器上的单次观测，不能当作性能基准；
- 同一固定样本的 Core ML 与 ONNX CPU 最终 scaled score 最大绝对差为
  `0.0011423826217651367`。这是后续样片门禁需要考虑的设备数值漂移，不代表业务精度。

项目运行时另有显式忽略测试
`features::smart_culling::arniqa_poc::tests::strict_project_runtime_smoke_test`，它使用 QRaw 内置
ONNX Runtime 1.22、项目现有严格硬件会话构造器，并禁止默认 CPU EP 回退。encoder 在默认图优化和
关闭图优化两种配置下都因仍有节点被分配给默认 CPU EP 而拒绝创建会话；三次核验后按项目失败处理
规则停止继续试错，没有放宽门禁、启用 CPU 回退或改用第二套生产运行时。因此：**Python 运行时通过
不等于 QRaw 项目运行时通过，当前结果明确阻止 ARNIQA 接入生产链路。**

本机 ONNX Runtime 没有 `DmlExecutionProvider`，所以 DirectML 只完成了常见算子和固定 shape 的静态
审阅。**不能宣称 Windows 已通过。** 生产准入前必须在目标 Windows GPU 上设置同样的禁用 CPU 回退
策略，真实运行两图，并记录节点接管、冷启动、热推理、内存、吞吐和数值差异。

## 5. 许可与数据限制

核实结果仍然不足以批准生产分发：

- ARNIQA 仓库根目录为 Apache-2.0，但 Release 页面仅说明发布 encoder 和各数据集回归头，没有为
  `.pth`、转换后的 ONNX 或衍生权重单独声明许可；
- encoder 使用 KADIS-700k 训练；KADID-10k/KADIS 官方页面只明确“freely available to the research
  community”，没有在本轮找到商业应用与衍生权重再分发条款；
- KonIQ-10k 官方页面同样只明确“freely available to the research community”；其图像来自
  YFCC100M，图片本身还有各自的来源许可；
- SPAQ 官方仓库提供数据库与模型，但仓库没有可由 GitHub License API 识别的许可证文件，本轮也没有
  找到对商业分发衍生权重的明确授权。

这不是法律意见。项目若要生产打包，必须先由权利方书面确认至少包括：商业使用、checkpoint 再分发、
转换 ONNX/衍生权重再分发、署名与第三方通知要求。没有明确结论时，当前两个 ONNX 只能保留为内部隔离
研究资产。

## 6. 复现

`export_and_verify.py` 会先严格校验四个官方下载文件的 SHA-256，再加载官方 `models/resnet.py`，比较
官方 PyTorch 输出、导出两张 ONNX、运行 ONNX checker，并执行 ONNX CPU 数值门禁。示例：

```sh
python3 export_and_verify.py \
  --official-repo /private/tmp/qraw-arniqa-poc/official-arniqa \
  --weights-dir /private/tmp/qraw-arniqa-poc \
  --output-dir /private/tmp/qraw-arniqa-poc/exported
```

脚本只接受固定 SHA 的官方资产。依赖版本和输出哈希发生变化时，必须重新审计，不能静默覆盖本目录
二进制。下一道门禁是三回归头在独立盲测样片上的相关性、误杀率与跨相机稳定性；没有盲测结果前，
不能仅凭本 POC 选择 SPAQ、KonIQ-10k 或 KADID-10k 作为生产回归头。
