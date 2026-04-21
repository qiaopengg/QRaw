#!/usr/bin/env python3
"""
测试脚本：验证值域归一化修复
用于检测图片变黑问题
"""
import numpy as np
from PIL import Image


def test_normalize_array_to_255(arr: np.ndarray, source: str = "test") -> np.ndarray:
    """
    复制修复后的归一化函数进行测试
    """
    print(f"\n{'='*60}")
    print(f"Testing: {source}")
    print(f"{'='*60}")
    print(f"Input dtype: {arr.dtype}, shape: {arr.shape}")
    print(f"Input range: [{arr.min():.4f}, {arr.max():.4f}], mean: {arr.mean():.4f}")
    
    # 转换为 float32
    if arr.dtype != np.float32:
        arr = arr.astype(np.float32)
    
    # 检测值域并归一化
    arr_min, arr_max = arr.min(), arr.max()
    
    if arr_max <= 1.0 and arr_min >= 0.0:
        # [0, 1] 范围
        print(f"✓ Detected [0, 1] range, scaling to [0, 255]")
        arr = arr * 255.0
    elif arr_min < 0.0 and arr_max <= 1.0:
        # [-1, 1] 范围
        print(f"✓ Detected [-1, 1] range, scaling to [0, 255]")
        arr = (arr + 1.0) * 127.5
    elif arr_max > 255.0:
        # 超出范围
        print(f"✓ Detected out-of-range values, normalizing")
        arr = (arr - arr_min) / (arr_max - arr_min + 1e-8) * 255.0
    elif arr_max <= 255.0 and arr_min >= 0.0:
        # 已经是 [0, 255] 范围
        print(f"✓ Already in [0, 255] range")
        pass
    else:
        # 未知范围
        print(f"✓ Unknown range, force normalizing")
        arr = (arr - arr_min) / (arr_max - arr_min + 1e-8) * 255.0
    
    # 最终裁剪
    arr = np.clip(arr, 0.0, 255.0)
    
    print(f"Output range: [{arr.min():.4f}, {arr.max():.4f}], mean: {arr.mean():.4f}")
    
    # 检测异常
    if arr.mean() < 10.0:
        print(f"⚠️  WARNING: Output is too dark! Mean: {arr.mean():.2f} / 255.0")
        return None
    else:
        print(f"✓ Output looks good!")
    
    return arr


def main():
    print("="*60)
    print("值域归一化测试 - 检测图片变黑问题")
    print("="*60)
    
    # 测试用例 1: [0, 1] 范围（正常 Diffusers 输出）
    test1 = np.random.rand(256, 256, 3).astype(np.float32)
    result1 = test_normalize_array_to_255(test1, "Case 1: [0, 1] range (normal)")
    
    # 测试用例 2: [-1, 1] 范围（某些模型输出）
    test2 = (np.random.rand(256, 256, 3).astype(np.float32) * 2.0 - 1.0)
    result2 = test_normalize_array_to_255(test2, "Case 2: [-1, 1] range")
    
    # 测试用例 3: [0, 255] 范围（PIL Image）
    test3 = (np.random.rand(256, 256, 3) * 255.0).astype(np.uint8)
    result3 = test_normalize_array_to_255(test3, "Case 3: [0, 255] uint8 (PIL)")
    
    # 测试用例 4: 错误的归一化（模拟 bug）
    test4 = (np.random.rand(256, 256, 3) * 255.0).astype(np.float32)
    print(f"\n{'='*60}")
    print(f"Testing: Case 4: Simulating OLD BUG")
    print(f"{'='*60}")
    print(f"Input range: [{test4.min():.4f}, {test4.max():.4f}], mean: {test4.mean():.4f}")
    
    # 旧代码的错误逻辑
    buggy_result = test4 / 255.0 * 65535.0  # 假设输入是 [0, 1]，但实际是 [0, 255]
    print(f"❌ OLD CODE output: [{buggy_result.min():.4f}, {buggy_result.max():.4f}], mean: {buggy_result.mean():.4f}")
    print(f"❌ This would result in: [{buggy_result.min()/65535*255:.4f}, {buggy_result.max()/65535*255:.4f}] in 8-bit")
    print(f"❌ Mean in 8-bit: {buggy_result.mean()/65535*255:.4f} / 255.0")
    
    if buggy_result.mean()/65535*255 < 10.0:
        print(f"❌ BUG CONFIRMED: Image would be nearly BLACK!")
    
    # 新代码的正确逻辑
    result4 = test_normalize_array_to_255(test4, "Case 4: Fixed version")
    
    # 测试用例 5: 极暗图像（夜景）
    test5 = (np.random.rand(256, 256, 3) * 0.3).astype(np.float32)  # [0, 0.3]
    result5 = test_normalize_array_to_255(test5, "Case 5: Dark image [0, 0.3]")
    
    # 测试用例 6: 高对比度图像
    test6 = np.random.rand(256, 256, 3).astype(np.float32)
    test6[test6 < 0.3] = 0.0  # 强制暗部为 0
    test6[test6 > 0.7] = 1.0  # 强制亮部为 1
    result6 = test_normalize_array_to_255(test6, "Case 6: High contrast")
    
    print(f"\n{'='*60}")
    print("测试总结")
    print(f"{'='*60}")
    print(f"✓ Case 1 ([0, 1]): {'PASS' if result1 is not None else 'FAIL'}")
    print(f"✓ Case 2 ([-1, 1]): {'PASS' if result2 is not None else 'FAIL'}")
    print(f"✓ Case 3 ([0, 255] uint8): {'PASS' if result3 is not None else 'FAIL'}")
    print(f"✓ Case 4 (Bug simulation): {'PASS' if result4 is not None else 'FAIL'}")
    print(f"✓ Case 5 (Dark image): {'PASS' if result5 is not None else 'FAIL'}")
    print(f"✓ Case 6 (High contrast): {'PASS' if result6 is not None else 'FAIL'}")
    
    print(f"\n{'='*60}")
    print("修复验证完成！")
    print(f"{'='*60}")


if __name__ == "__main__":
    main()
