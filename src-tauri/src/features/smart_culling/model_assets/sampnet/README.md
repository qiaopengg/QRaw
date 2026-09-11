# SAMP-Net 构图候选：离线网络与转换验证

2026-09-11：官方网络、ONNX转换及项目严格CoreML的19组数值对照已通过。
**这些是工程等价验证，尚未完成构图通过线、准确率验收或评分接入。**
没有新增应用依赖，没有把权重放进仓库或 Tauri 资源。光学/构图联合项仍为 +0。

## 官方来源与本地权重

- [CADB / SAMP-Net 官方仓库](https://github.com/bcmi/Image-Composition-Assessment-Dataset-CADB)
- 固定提交：`b8f80be379d4a4caf9d045db3187cb60bcaca583`
- 代码为 MIT；本目录 `LICENSE-MIT.txt` 保留 BCMI 署名及完整许可。
- [官方输入实现](https://github.com/bcmi/Image-Composition-Assessment-Dataset-CADB/blob/b8f80be379d4a4caf9d045db3187cb60bcaca583/SAMPNet/cadb_dataset.py) SHA-256：
  `24656f1ce9b2c135415e2633ba9570404cfb6cca45f6e892c9bc6c6a9e2753d9`。
- 通过官方 README 的 Dropbox 链接取得 `samp_net.pth`，183,416,305 字节，SHA-256：
  `d3d02f36967734fd6f0d2770380a04b777b5cd800a9ba68d4ccc9656f6fef603`。
  `torch.load(..., weights_only=True, map_location='cpu')` 读到 136 个有限张量，
  共 45,841,876 个张量元素（含缓冲区）；现已严格载入完整官方网络并完成离线推理。

权重、六份官方源码快照及来源哈希只保存在独立结果目录：
`/Users/qiaopeng/Desktop/测试/QRaw_人像其他功能开发_第001批/光学构图接入与发布核查_v1/research`。
旧阶段下载未完成的记录保持原样；本次成功不覆盖旧失败。
仓库代码许可不自动解除项目要求的权重/训练资料再分发审核，正式发布仍未获验收。

## 输入和输出边界

`preprocess.py` 接收已正确朝向的 uint8 RGB 整图，生成两份连续 float32 张量：

| 输入 | 形状 | 处理 |
| --- | --- | --- |
| `image` | `[1,3,224,224]` | 按官方 PIL 双线性缩放整图，不居中裁剪；RGB/255 后使用 ImageNet mean/std |
| `saliency` | `[1,1,224,224]` | 在完整原尺寸输入上执行官方频谱残差流程，再缩放至 224；不能从模型缩略图或脸框生成 |

显著图不是额外的学习模型。保留官方 RGB 转灰度、1/6 面积缩放、FFT、3×3 均值、
9×9 高斯、95% 分位截断和原除数顺序。小于 6 像素、非 RGB8 或非法张量明确拒绝，
不填充假显著图；输入数组不修改。该频谱流程仅服务构图，不是已暂停的清晰度/噪声探针。

官方网络输出 8 个 pattern logits、6 个辅助属性及 5 档构图分布。活动配置中的属性为
RuleOfThirds、BalacingElements、DoF、Object、Symmetry、Repetition，**没有独立曝光或色彩质量输出**。
不能将这些属性、构图均分或 Apple 整体审美分当作光学通过；构图分布也不是表情五档或照片最终星级。
本阶段没有添加通过阈值、假判定器或第二套评分入口。

## 独立环境与实现

用户已明确回复“允许安装验证依赖”。只在下列结果目录的venv中安装
`torchvision==0.23.0`和`einops==0.8.1`；复用原有PyTorch2.8.0及ONNX工具。
wheel先按官方PyPI元数据核对哈希，再以`--no-deps --no-index`本地安装，未修改全局或应用依赖。
安装和后续完整证据目录：
`/Users/qiaopeng/Desktop/测试/QRaw_人像其他功能开发_第001批/SAMPNet网络与转换_v1`。

`official.py`先核对固定源码与权重哈希，执行原始推理定义，排除训练路径断言和主入口；
以`pretrained=False`构建后，`load_state_dict(strict=True)`载入全部136个官方张量再执行。
没有使用随机网络或另一份ResNet权重产生结果。`network_baseline.py`复用既有16组输入和3张隔离副本，
分别与官方torchvision变换和显著图函数逐值对照，冻结38份输入张量及全部3类输出。

`export_network.py`复用官方模型和冻结基线，按静态224×224双输入导出opset17。
`export_pooling.py`使用官方自适应池化的精确分箱，1D池化增加单元素维度执行2D池化；
`export_geometry.py`在跟踪前用原torch算子预先生成固定三角索引。
`export_graph.py`复用ORT BASIC常量折叠，按已知形状将41个Flatten改写成Reshape；
未量化、重训、更换权重或修改任何质量阈值。参见[ORT图优化说明](https://onnxruntime.ai/docs/performance/model-optimizations/graph-optimizations.html)。
中间`traced.onnx`、`constant_folded.onnx`和最终图分别保存，旧导出失败不覆盖。

`sampnet_gpu_test.rs`与`input.rs`只在macOS测试构建中启用，复用项目原有
`gpu_session_with_optimization`、契约检查与哈希校验。CoreML为MLProgram、CPUAndGPU，
ORT CPU EP回退禁用；这证明整张图通过项目provider门禁，不证明每个算子物理上都在GPU执行。
没有新增正式评分入口或把验证模型复制到运行资源。

## 已执行验证与后续

以下为测试入口；Python需使用上述venv的解释器。结果路径必须新建，同名目录/报告拒绝覆盖：

```sh
SAMP_RESULT='/Users/qiaopeng/Desktop/测试/QRaw_人像其他功能开发_第001批/SAMPNet网络与转换_v1'
"$SAMP_RESULT/venv/bin/python" -m unittest discover -s src-tauri/src/features/smart_culling/model_assets/sampnet -p 'test_*.py' -v
cargo +1.96.1 test --manifest-path src-tauri/Cargo.toml features::smart_culling -- --quiet
cargo +1.96.1 fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo +1.96.1 check --manifest-path src-tauri/Cargo.toml --release --offline
```

`SAMP_RESULT`指上面的结果目录。Python17项回归、Rust智能选图322项通过/14项忽略，
fmt和Release离线check通过。显式GPU测试另外1项通过，执行了全部19组；对应命令及环境变量
在`gpu-prepared-COMMAND.json`，它读取`baseline/`、`export_prepared/`并写新报告。

| 核验 | 样本数 | 最大绝对差 | 事前容差 |
| --- | ---: | ---: | --- |
| 两份输入与官方 | 19 | 0 | 逐值相同 |
| ONNX与官方网络 | 19 | 2.14577e-6 | atol=1e-4，rtol=1e-5 |
| 项目CoreML与官方网络 | 19 | 2.08617e-6 | atol=1e-3，rtol=1e-5 |

最终`export_prepared/samp_net.onnx`为183,506,326字节，SHA-256：
`e7bf8240854da50e9f44cc5710cf69baf9d4139c3c8b312576bb6924fc654a3a`。
`GPU_PREPARED.json`绑定同一模型哈希。两次原始导出失败、首次GPU不支持节点失败、
日志回调崩溃及中间转换图全部保留，详见结果目录`REPORT.md`和`VALIDATION.json`。

光学独立判定、构图通过线及各自的实拍验收尚未完成，不能据本目录宣称自动判定可用。
19组中16组是程序构造输入、3张是既有合成图片副本，不能充当19张独立摄影验收照。
下一步需要先固定两项各自的验证含义、通过线与独立验收方案；不得用现有分布或构图属性
兼充光学通过。眼态/表情正式发布、权重许可及跨平台也未验收，`release_ready=false`。
