#!/usr/bin/env python3
"""
完整 Pipeline 测试脚本
测试所有功能模块的集成
"""
import numpy as np
from PIL import Image
import sys
from pathlib import Path

# 添加当前目录到路径
sys.path.insert(0, str(Path(__file__).parent))

def test_color_alignment():
    """测试色彩对齐模块"""
    print("\n" + "="*60)
    print("测试色彩对齐模块")
    print("="*60)
    
    try:
        from color_alignment import (
            rgb_to_luma,
            luminance_aware_mapping,
            apply_tone_curve,
            preserve_dynamic_range,
            apply_color_alignment,
        )
        
        # 创建测试图像
        ai_result = np.random.rand(512, 512, 3).astype(np.float32) * 255
        original = np.random.rand(512, 512, 3).astype(np.float32) * 255
        
        # 测试各个功能
        print("✓ rgb_to_luma")
        luma = rgb_to_luma(ai_result)
        assert luma.shape == (512, 512), f"Expected shape (512, 512), got {luma.shape}"
        
        print("✓ luminance_aware_mapping")
        result1 = luminance_aware_mapping(ai_result, original, strength=0.3)
        assert result1.shape == ai_result.shape
        assert result1.min() >= 0 and result1.max() <= 255
        
        print("✓ apply_tone_curve")
        result2 = apply_tone_curve(ai_result, curve_type="s_curve", strength=0.5)
        assert result2.shape == ai_result.shape
        
        print("✓ preserve_dynamic_range")
        result3 = preserve_dynamic_range(ai_result, original, preserve_ratio=0.3)
        assert result3.shape == ai_result.shape
        
        print("✓ apply_color_alignment (full)")
        result4 = apply_color_alignment(ai_result, original, mode="full")
        assert result4.shape == ai_result.shape
        
        print("\n✅ 色彩对齐模块测试通过！")
        return True
        
    except ImportError as e:
        print(f"\n❌ 色彩对齐模块导入失败: {e}")
        return False
    except Exception as e:
        print(f"\n❌ 色彩对齐模块测试失败: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_raw_fusion():
    """测试 RAW 融合模块"""
    print("\n" + "="*60)
    print("测试 RAW 融合模块")
    print("="*60)
    
    try:
        from raw_fusion import (
            extract_tone_curve_from_adjustments,
            blend_with_raw_tone_curve,
            preserve_highlight_details,
            preserve_shadow_details,
            apply_raw_fusion,
        )
        
        # 创建测试图像
        ai_result = np.random.rand(512, 512, 3).astype(np.float32) * 255
        raw_processed = np.random.rand(512, 512, 3).astype(np.float32) * 255
        
        # 测试 tone curve 提取
        print("✓ extract_tone_curve_from_adjustments")
        adjustments = {
            "exposure": 50,
            "contrast": 20,
            "highlights": -30,
            "shadows": 40,
        }
        curve = extract_tone_curve_from_adjustments(adjustments)
        assert curve.shape == (256,), f"Expected shape (256,), got {curve.shape}"
        
        # 测试融合
        print("✓ blend_with_raw_tone_curve (luminance)")
        result1 = blend_with_raw_tone_curve(ai_result, raw_processed, blend_strength=0.5, mode="luminance")
        assert result1.shape == ai_result.shape
        
        print("✓ blend_with_raw_tone_curve (adaptive)")
        result2 = blend_with_raw_tone_curve(ai_result, raw_processed, blend_strength=0.5, mode="adaptive")
        assert result2.shape == ai_result.shape
        
        # 测试高光保护
        print("✓ preserve_highlight_details")
        result3 = preserve_highlight_details(ai_result, raw_processed, threshold=200.0, strength=0.6)
        assert result3.shape == ai_result.shape
        
        # 测试阴影保护
        print("✓ preserve_shadow_details")
        result4 = preserve_shadow_details(ai_result, raw_processed, threshold=50.0, strength=0.5)
        assert result4.shape == ai_result.shape
        
        # 测试完整流程
        print("✓ apply_raw_fusion (full)")
        result5 = apply_raw_fusion(
            ai_result,
            raw_processed=raw_processed,
            current_adjustments=adjustments,
            blend_strength=0.5,
            blend_mode="luminance",
        )
        assert result5.shape == ai_result.shape
        
        print("\n✅ RAW 融合模块测试通过！")
        return True
        
    except ImportError as e:
        print(f"\n❌ RAW 融合模块导入失败: {e}")
        return False
    except Exception as e:
        print(f"\n❌ RAW 融合模块测试失败: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_value_normalization():
    """测试值域归一化"""
    print("\n" + "="*60)
    print("测试值域归一化")
    print("="*60)
    
    try:
        from app_fixed import _normalize_array_to_255
        
        # 测试用例 1: [0, 1] 范围
        test1 = np.random.rand(256, 256, 3).astype(np.float32)
        result1 = _normalize_array_to_255(test1, "test1")
        assert result1.min() >= 0 and result1.max() <= 255
        assert result1.mean() > 100, "Result too dark"
        print("✓ [0, 1] 范围测试通过")
        
        # 测试用例 2: [-1, 1] 范围
        test2 = (np.random.rand(256, 256, 3).astype(np.float32) * 2.0 - 1.0)
        result2 = _normalize_array_to_255(test2, "test2")
        assert result2.min() >= 0 and result2.max() <= 255
        print("✓ [-1, 1] 范围测试通过")
        
        # 测试用例 3: [0, 255] uint8
        test3 = (np.random.rand(256, 256, 3) * 255.0).astype(np.uint8)
        result3 = _normalize_array_to_255(test3, "test3")
        assert result3.min() >= 0 and result3.max() <= 255
        print("✓ [0, 255] uint8 测试通过")
        
        print("\n✅ 值域归一化测试通过！")
        return True
        
    except Exception as e:
        print(f"\n❌ 值域归一化测试失败: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_integration():
    """测试完整集成"""
    print("\n" + "="*60)
    print("测试完整集成")
    print("="*60)
    
    try:
        # 检查所有模块是否可导入
        print("检查模块导入...")
        
        from app_fixed import (
            _normalize_array_to_255,
            _ensure_rgb,
            _blend_tiles,
            HAS_COLOR_ALIGNMENT,
            HAS_RAW_FUSION,
        )
        print("✓ app_fixed 模块导入成功")
        
        if HAS_COLOR_ALIGNMENT:
            from color_alignment import apply_color_alignment
            print("✓ color_alignment 模块可用")
        else:
            print("⚠️  color_alignment 模块不可用")
        
        if HAS_RAW_FUSION:
            from raw_fusion import apply_raw_fusion
            print("✓ raw_fusion 模块可用")
        else:
            print("⚠️  raw_fusion 模块不可用")
        
        # 模拟完整 pipeline
        print("\n模拟完整 pipeline...")
        
        # 1. 创建测试图像
        ai_result = np.random.rand(512, 512, 3).astype(np.float32) * 255
        original = np.random.rand(512, 512, 3).astype(np.float32) * 255
        
        # 2. 值域归一化
        ai_result = _normalize_array_to_255(ai_result, "ai_result")
        original = _normalize_array_to_255(original, "original")
        print("✓ 值域归一化完成")
        
        # 3. 色彩对齐
        if HAS_COLOR_ALIGNMENT:
            ai_result = apply_color_alignment(
                ai_result,
                original,
                mode="full",
                luminance_strength=0.3,
                tone_curve_strength=0.5,
                dynamic_range_preserve=0.3,
            )
            print("✓ 色彩对齐完成")
        
        # 4. RAW 融合
        if HAS_RAW_FUSION:
            ai_result = apply_raw_fusion(
                ai_result,
                raw_processed=original,
                current_adjustments={"exposure": 0, "contrast": 0},
                blend_strength=0.5,
                blend_mode="luminance",
            )
            print("✓ RAW 融合完成")
        
        # 5. 验证最终结果
        assert ai_result.shape == (512, 512, 3)
        assert ai_result.min() >= 0 and ai_result.max() <= 255
        assert ai_result.mean() > 10, "Final result too dark"
        print("✓ 最终结果验证通过")
        
        print("\n✅ 完整集成测试通过！")
        return True
        
    except Exception as e:
        print(f"\n❌ 完整集成测试失败: {e}")
        import traceback
        traceback.print_exc()
        return False


def main():
    """运行所有测试"""
    print("="*60)
    print("RapidRAW 风格迁移完整 Pipeline 测试")
    print("="*60)
    
    results = {
        "值域归一化": test_value_normalization(),
        "色彩对齐": test_color_alignment(),
        "RAW 融合": test_raw_fusion(),
        "完整集成": test_integration(),
    }
    
    print("\n" + "="*60)
    print("测试总结")
    print("="*60)
    
    for name, passed in results.items():
        status = "✅ PASS" if passed else "❌ FAIL"
        print(f"{status} - {name}")
    
    all_passed = all(results.values())
    
    print("\n" + "="*60)
    if all_passed:
        print("🎉 所有测试通过！")
        print("="*60)
        return 0
    else:
        print("⚠️  部分测试失败，请检查上述错误信息")
        print("="*60)
        return 1


if __name__ == "__main__":
    sys.exit(main())
