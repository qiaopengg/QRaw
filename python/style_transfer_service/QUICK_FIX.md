# 色彩丢失问题 - 快速修复方案

## 问题描述

风格迁移输出变成纯明度图片（灰度图），色彩信息丢失。

## 可能原因

1. **色彩对齐模块问题** - `luminance_aware_mapping()` 使用 LAB 色彩空间转换可能有问题
2. **Pipeline 输出问题** - SDXL 输出可能已经是灰度
3. **RAW 融合问题** - 融合过程中色彩丢失

## 快速修复步骤

### 方案 1: 禁用色彩对齐（推荐）

在前端发送请求时，设置：

```typescript
{
  enable_color_alignment: false,  // 禁用色彩对齐
  enable_raw_fusion: false,       // 禁用 RAW 融合
}
```

### 方案 2: 修改默认参数

编辑 `app.py`，找到 `StyleTransferRequest` 类，修改默认值：

```python
class StyleTransferRequest(BaseModel):
    # ...
    enable_color_alignment: bool = Field(False, alias="enableColorAlignment")  # 改为 False
    enable_raw_fusion: bool = Field(False, alias="enableRawFusion")  # 改为 False
```

### 方案 3: 使用简化版色彩对齐

已经在 `color_alignment.py` 中临时禁用了 LAB 转换，改用简化版本：

```python
def luminance_aware_mapping(...):
    # 🔧 临时禁用 LAB 转换，直接使用简化版本
    return _luminance_aware_mapping_simple(ai_result, original_img, strength)
```

## 诊断工具

运行诊断脚本检查输出图像：

```bash
cd python/style_transfer_service
python3 diagnose_color_loss.py
```

或者检查特定图像：

```bash
python3 diagnose_color_loss.py /path/to/output.tiff
```

## 重启服务

修改后需要重启 Python 服务：

```bash
# 停止当前服务（Ctrl+C）
# 重新启动
cd python/style_transfer_service
export QRAW_DEBUG="1"
python3 app.py
```

## 验证修复

1. 重启服务
2. 执行新的风格迁移
3. 检查输出是否有色彩
4. 如果仍然是灰度，说明问题在 Pipeline 输出，而不是后处理

## 下一步调试

如果禁用色彩对齐后仍然是灰度图，需要检查：

1. **SDXL Pipeline 输出** - 在 `_run_single()` 中添加调试：

   ```python
   print(f"[DEBUG] Pipeline output type: {type(image)}")
   print(f"[DEBUG] Pipeline output mode: {image.mode if hasattr(image, 'mode') else 'N/A'}")
   arr = np.array(image)
   print(f"[DEBUG] Pipeline output shape: {arr.shape}")
   print(f"[DEBUG] RGB means: R={arr[:,:,0].mean()}, G={arr[:,:,1].mean()}, B={arr[:,:,2].mean()}")
   ```

2. **IP-Adapter 问题** - 当前 IP-Adapter 未加载，可能影响色彩生成

3. **ControlNet 问题** - Canny 边缘检测是灰度的，但不应该影响最终输出

---

**创建时间**: 2026-04-21  
**状态**: 待验证
