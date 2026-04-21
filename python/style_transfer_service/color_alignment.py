"""
色彩对齐系统
实现文档第 7.2 节：Luminance-aware Mapping + 曲线微调
"""
import numpy as np
from typing import Optional, Tuple
from PIL import Image

try:
    import cv2
    HAS_CV2 = True
except ImportError:
    HAS_CV2 = False


def rgb_to_luma(rgb: np.ndarray) -> np.ndarray:
    """
    将 RGB 转换为亮度（Luma）
    使用 Rec.709 标准：Y = 0.2126*R + 0.7152*G + 0.0722*B
    
    Args:
        rgb: [H, W, 3] float32 数组，范围 [0, 255]
    
    Returns:
        [H, W] float32 数组，范围 [0, 255]
    """
    if rgb.shape[2] != 3:
        raise ValueError(f"Expected RGB image with 3 channels, got {rgb.shape[2]}")
    
    # Rec.709 权重
    weights = np.array([0.2126, 0.7152, 0.0722], dtype=np.float32)
    luma = np.dot(rgb, weights)
    
    return luma


def replace_luma(rgb: np.ndarray, new_luma: np.ndarray) -> np.ndarray:
    """
    替换 RGB 图像的亮度，保持色彩
    
    Args:
        rgb: [H, W, 3] float32 数组，范围 [0, 255]
        new_luma: [H, W] float32 数组，范围 [0, 255]
    
    Returns:
        [H, W, 3] float32 数组，范围 [0, 255]
    """
    old_luma = rgb_to_luma(rgb)
    
    # 避免除零
    old_luma_safe = np.maximum(old_luma, 1e-6)
    
    # 计算缩放因子
    scale = new_luma / old_luma_safe
    scale = np.clip(scale, 0.0, 10.0)  # 限制极端值
    
    # 应用到每个通道
    result = rgb * scale[:, :, np.newaxis]
    
    return np.clip(result, 0.0, 255.0)


def luminance_aware_mapping(
    ai_result: np.ndarray,
    original_img: np.ndarray,
    strength: float = 0.3,
    preserve_highlights: bool = True,
    preserve_shadows: bool = True,
) -> np.ndarray:
    """
    文档第 7.2 节：Luminance-aware Mapping（低权重）
    
    在保持 AI 风格的同时，部分恢复原图的亮度分布
    
    Args:
        ai_result: AI 生成结果，[H, W, 3] float32，范围 [0, 255]
        original_img: 原始图像，[H, W, 3] float32，范围 [0, 255]
        strength: 原图亮度的混合强度，0.0-1.0，文档建议低权重（0.2-0.4）
        preserve_highlights: 是否保护高光细节
        preserve_shadows: 是否保护阴影层次
    
    Returns:
        [H, W, 3] float32 数组，范围 [0, 255]
    """
    if not HAS_CV2:
        # 如果没有 OpenCV，使用简化版本
        return _luminance_aware_mapping_simple(ai_result, original_img, strength)
    
    # 转换到 LAB 色彩空间
    ai_lab = cv2.cvtColor(ai_result.astype(np.uint8), cv2.COLOR_RGB2LAB).astype(np.float32)
    orig_lab = cv2.cvtColor(original_img.astype(np.uint8), cv2.COLOR_RGB2LAB).astype(np.float32)
    
    # 提取亮度通道（L）
    ai_l = ai_lab[:, :, 0]
    orig_l = orig_lab[:, :, 0]
    
    # 基础混合
    blended_l = ai_l * (1.0 - strength) + orig_l * strength
    
    # 高光保护（保留原图的高光细节）
    if preserve_highlights:
        highlight_mask = (orig_l > 200).astype(np.float32)
        highlight_strength = highlight_mask * 0.5  # 高光区域增强保护
        blended_l = blended_l * (1.0 - highlight_strength) + orig_l * highlight_strength
    
    # 阴影保护（保留原图的阴影层次）
    if preserve_shadows:
        shadow_mask = (orig_l < 50).astype(np.float32)
        shadow_strength = shadow_mask * 0.4  # 阴影区域增强保护
        blended_l = blended_l * (1.0 - shadow_strength) + orig_l * shadow_strength
    
    # 更新亮度通道
    ai_lab[:, :, 0] = np.clip(blended_l, 0.0, 255.0)
    
    # 转换回 RGB
    result = cv2.cvtColor(ai_lab.astype(np.uint8), cv2.COLOR_LAB2RGB).astype(np.float32)
    
    return result


def _luminance_aware_mapping_simple(
    ai_result: np.ndarray,
    original_img: np.ndarray,
    strength: float,
) -> np.ndarray:
    """
    简化版本（不依赖 OpenCV）
    直接在 RGB 空间混合亮度
    """
    ai_luma = rgb_to_luma(ai_result)
    orig_luma = rgb_to_luma(original_img)
    
    # 混合亮度
    blended_luma = ai_luma * (1.0 - strength) + orig_luma * strength
    
    # 替换 AI 结果的亮度
    result = replace_luma(ai_result, blended_luma)
    
    return result


def apply_tone_curve(
    img: np.ndarray,
    curve_points: Optional[np.ndarray] = None,
    curve_type: str = "s_curve",
    strength: float = 0.5,
) -> np.ndarray:
    """
    文档第 7.2 节：曲线微调（Tone Curve）
    
    应用色调曲线来微调图像的对比度和色调
    
    Args:
        img: [H, W, 3] float32 数组，范围 [0, 255]
        curve_points: 自定义曲线点，[N, 2] 数组，x 和 y 都在 [0, 255]
        curve_type: 预设曲线类型：'s_curve', 'contrast', 'brighten', 'darken'
        strength: 曲线强度，0.0-1.0
    
    Returns:
        [H, W, 3] float32 数组，范围 [0, 255]
    """
    if curve_points is None:
        # 使用预设曲线
        curve_points = _get_preset_curve(curve_type)
    
    # 创建查找表（LUT）
    lut = _create_curve_lut(curve_points)
    
    # 应用强度
    if strength < 1.0:
        identity = np.arange(256, dtype=np.float32)
        lut = lut * strength + identity * (1.0 - strength)
    
    # 应用 LUT
    lut = np.clip(lut, 0, 255).astype(np.uint8)
    result = lut[img.astype(np.uint8)]
    
    return result.astype(np.float32)


def _get_preset_curve(curve_type: str) -> np.ndarray:
    """
    获取预设曲线点
    
    Returns:
        [N, 2] 数组，x 和 y 都在 [0, 255]
    """
    if curve_type == "s_curve":
        # S 曲线：增强对比度
        return np.array([
            [0, 0],
            [64, 48],      # 压暗阴影
            [128, 128],    # 中间调不变
            [192, 208],    # 提亮高光
            [255, 255],
        ], dtype=np.float32)
    
    elif curve_type == "contrast":
        # 对比度增强
        return np.array([
            [0, 0],
            [64, 40],
            [128, 128],
            [192, 216],
            [255, 255],
        ], dtype=np.float32)
    
    elif curve_type == "brighten":
        # 整体提亮
        return np.array([
            [0, 20],
            [64, 80],
            [128, 148],
            [192, 216],
            [255, 255],
        ], dtype=np.float32)
    
    elif curve_type == "darken":
        # 整体压暗
        return np.array([
            [0, 0],
            [64, 40],
            [128, 108],
            [192, 176],
            [255, 235],
        ], dtype=np.float32)
    
    else:
        # 线性（无变化）
        return np.array([
            [0, 0],
            [255, 255],
        ], dtype=np.float32)


def _create_curve_lut(curve_points: np.ndarray) -> np.ndarray:
    """
    从曲线点创建 256 级查找表
    
    Args:
        curve_points: [N, 2] 数组，x 和 y 都在 [0, 255]
    
    Returns:
        [256] 数组
    """
    # 确保点按 x 排序
    curve_points = curve_points[curve_points[:, 0].argsort()]
    
    # 插值生成 LUT
    x = curve_points[:, 0]
    y = curve_points[:, 1]
    
    lut = np.interp(np.arange(256), x, y)
    
    return lut.astype(np.float32)


def histogram_matching(
    source: np.ndarray,
    reference: np.ndarray,
    strength: float = 0.3,
) -> np.ndarray:
    """
    直方图匹配（文档提到"放弃强 Histogram Matching"，所以这里是弱版本）
    
    Args:
        source: 源图像，[H, W, 3] float32，范围 [0, 255]
        reference: 参考图像，[H, W, 3] float32，范围 [0, 255]
        strength: 匹配强度，0.0-1.0，文档建议低权重
    
    Returns:
        [H, W, 3] float32 数组，范围 [0, 255]
    """
    result = source.copy()
    
    for c in range(3):
        # 计算累积分布函数（CDF）
        src_hist, bins = np.histogram(source[:, :, c].flatten(), 256, [0, 256])
        ref_hist, _ = np.histogram(reference[:, :, c].flatten(), 256, [0, 256])
        
        src_cdf = src_hist.cumsum()
        ref_cdf = ref_hist.cumsum()
        
        # 归一化
        src_cdf = src_cdf / src_cdf[-1]
        ref_cdf = ref_cdf / ref_cdf[-1]
        
        # 创建映射
        lut = np.zeros(256, dtype=np.uint8)
        for i in range(256):
            # 找到最接近的参考值
            diff = np.abs(ref_cdf - src_cdf[i])
            lut[i] = np.argmin(diff)
        
        # 应用映射
        matched = lut[source[:, :, c].astype(np.uint8)]
        
        # 混合原图和匹配结果
        result[:, :, c] = (
            source[:, :, c] * (1.0 - strength) +
            matched.astype(np.float32) * strength
        )
    
    return np.clip(result, 0.0, 255.0)


def preserve_dynamic_range(
    ai_result: np.ndarray,
    original_img: np.ndarray,
    preserve_ratio: float = 0.3,
) -> np.ndarray:
    """
    文档第 7.3 节：保持动态范围
    
    确保 AI 结果保留原图的动态范围特性
    
    Args:
        ai_result: AI 生成结果，[H, W, 3] float32，范围 [0, 255]
        original_img: 原始图像，[H, W, 3] float32，范围 [0, 255]
        preserve_ratio: 保留原图动态范围的比例，0.0-1.0
    
    Returns:
        [H, W, 3] float32 数组，范围 [0, 255]
    """
    # 计算原图的动态范围
    orig_min = original_img.min(axis=(0, 1))
    orig_max = original_img.max(axis=(0, 1))
    orig_range = orig_max - orig_min
    
    # 计算 AI 结果的动态范围
    ai_min = ai_result.min(axis=(0, 1))
    ai_max = ai_result.max(axis=(0, 1))
    ai_range = ai_max - ai_min
    
    # 目标动态范围（混合）
    target_min = ai_min * (1.0 - preserve_ratio) + orig_min * preserve_ratio
    target_max = ai_max * (1.0 - preserve_ratio) + orig_max * preserve_ratio
    
    # 重新映射 AI 结果
    result = ai_result.copy()
    for c in range(3):
        if ai_range[c] > 1e-6:
            # 归一化到 [0, 1]
            normalized = (result[:, :, c] - ai_min[c]) / ai_range[c]
            # 映射到目标范围
            result[:, :, c] = normalized * (target_max[c] - target_min[c]) + target_min[c]
    
    return np.clip(result, 0.0, 255.0)


def apply_color_alignment(
    ai_result: np.ndarray,
    original_img: np.ndarray,
    mode: str = "full",
    luminance_strength: float = 0.3,
    tone_curve_strength: float = 0.5,
    dynamic_range_preserve: float = 0.3,
) -> np.ndarray:
    """
    完整的色彩对齐流程（文档第 7.2 节）
    
    Args:
        ai_result: AI 生成结果，[H, W, 3] float32，范围 [0, 255]
        original_img: 原始图像，[H, W, 3] float32，范围 [0, 255]
        mode: 对齐模式，'full', 'luminance_only', 'tone_only', 'none'
        luminance_strength: 亮度映射强度
        tone_curve_strength: 曲线调整强度
        dynamic_range_preserve: 动态范围保留比例
    
    Returns:
        [H, W, 3] float32 数组，范围 [0, 255]
    """
    if mode == "none":
        return ai_result
    
    result = ai_result.copy()
    
    # 步骤 1: 动态范围保护
    if mode in ["full"] and dynamic_range_preserve > 0:
        result = preserve_dynamic_range(result, original_img, dynamic_range_preserve)
    
    # 步骤 2: 亮度映射
    if mode in ["full", "luminance_only"] and luminance_strength > 0:
        result = luminance_aware_mapping(
            result,
            original_img,
            strength=luminance_strength,
            preserve_highlights=True,
            preserve_shadows=True,
        )
    
    # 步骤 3: 曲线微调
    if mode in ["full", "tone_only"] and tone_curve_strength > 0:
        result = apply_tone_curve(
            result,
            curve_type="s_curve",
            strength=tone_curve_strength,
        )
    
    return result


# 测试函数
if __name__ == "__main__":
    print("色彩对齐系统测试")
    print("=" * 60)
    
    # 创建测试图像
    test_ai = np.random.rand(512, 512, 3).astype(np.float32) * 255
    test_orig = np.random.rand(512, 512, 3).astype(np.float32) * 255
    
    # 测试各个功能
    print("✓ rgb_to_luma")
    luma = rgb_to_luma(test_ai)
    print(f"  Luma shape: {luma.shape}, range: [{luma.min():.2f}, {luma.max():.2f}]")
    
    print("✓ luminance_aware_mapping")
    result1 = luminance_aware_mapping(test_ai, test_orig, strength=0.3)
    print(f"  Result shape: {result1.shape}, range: [{result1.min():.2f}, {result1.max():.2f}]")
    
    print("✓ apply_tone_curve")
    result2 = apply_tone_curve(test_ai, curve_type="s_curve", strength=0.5)
    print(f"  Result shape: {result2.shape}, range: [{result2.min():.2f}, {result2.max():.2f}]")
    
    print("✓ preserve_dynamic_range")
    result3 = preserve_dynamic_range(test_ai, test_orig, preserve_ratio=0.3)
    print(f"  Result shape: {result3.shape}, range: [{result3.min():.2f}, {result3.max():.2f}]")
    
    print("✓ apply_color_alignment (full)")
    result4 = apply_color_alignment(test_ai, test_orig, mode="full")
    print(f"  Result shape: {result4.shape}, range: [{result4.min():.2f}, {result4.max():.2f}]")
    
    print("\n所有测试通过！")
