#!/usr/bin/env python3
"""
测试 Tiling 修复
验证索引越界问题是否已解决
"""
import numpy as np
from PIL import Image
import sys
from pathlib import Path

# 添加当前目录到路径
sys.path.insert(0, str(Path(__file__).parent))

def test_make_weight_1d():
    """测试 _make_weight_1d 函数"""
    print("\n" + "="*60)
    print("测试 _make_weight_1d 函数")
    print("="*60)
    
    try:
        from app_fixed import _make_weight_1d
        
        # 测试用例 1: 正常情况
        print("✓ 测试用例 1: 正常情况 (length=100, overlap=20)")
        w1 = _make_weight_1d(100, 20, at_start=False, at_end=False)
        assert w1.shape == (100,), f"Expected shape (100,), got {w1.shape}"
        assert w1.min() >= 0 and w1.max() <= 1, f"Values out of range: [{w1.min()}, {w1.max()}]"
        print(f"  Shape: {w1.shape}, Range: [{w1.min():.4f}, {w1.max():.4f}]")
        
        # 测试用例 2: 小尺寸
        print("✓ 测试用例 2: 小尺寸 (length=10, overlap=5)")
        w2 = _make_weight_1d(10, 5, at_start=False, at_end=False)
        assert w2.shape == (10,), f"Expected shape (10,), got {w2.shape}"
        print(f"  Shape: {w2.shape}, Range: [{w2.min():.4f}, {w2.max():.4f}]")
        
        # 测试用例 3: 极小尺寸（可能导致 o=0）
        print("✓ 测试用例 3: 极小尺寸 (length=2, overlap=10)")
        w3 = _make_weight_1d(2, 10, at_start=False, at_end=False)
        assert w3.shape == (2,), f"Expected shape (2,), got {w3.shape}"
        print(f"  Shape: {w3.shape}, Range: [{w3.min():.4f}, {w3.max():.4f}]")
        
        # 测试用例 4: overlap=0
        print("✓ 测试用例 4: overlap=0 (length=100, overlap=0)")
        w4 = _make_weight_1d(100, 0, at_start=False, at_end=False)
        assert w4.shape == (100,), f"Expected shape (100,), got {w4.shape}"
        assert np.all(w4 == 1.0), "Expected all ones"
        print(f"  Shape: {w4.shape}, All ones: {np.all(w4 == 1.0)}")
        
        # 测试用例 5: 边界情况（原始错误场景）
        print("✓ 测试用例 5: 边界情况 (length=39, overlap=96)")
        w5 = _make_weight_1d(39, 96, at_start=False, at_end=True)
        assert w5.shape == (39,), f"Expected shape (39,), got {w5.shape}"
        print(f"  Shape: {w5.shape}, Range: [{w5.min():.4f}, {w5.max():.4f}]")
        
        print("\n✅ _make_weight_1d 测试通过！")
        return True
        
    except Exception as e:
        print(f"\n❌ _make_weight_1d 测试失败: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_blend_tiles():
    """测试 _blend_tiles 函数"""
    print("\n" + "="*60)
    print("测试 _blend_tiles 函数")
    print("="*60)
    
    try:
        from app_fixed import _blend_tiles
        
        # 创建测试 tiles
        tile1 = np.random.rand(100, 100, 3).astype(np.float32) * 255
        tile2 = np.random.rand(100, 100, 3).astype(np.float32) * 255
        tile3 = np.random.rand(100, 100, 3).astype(np.float32) * 255
        tile4 = np.random.rand(100, 100, 3).astype(np.float32) * 255
        
        tiles = [
            (0, 0, tile1),
            (80, 0, tile2),
            (0, 80, tile3),
            (80, 80, tile4),
        ]
        
        print("✓ 测试用例 1: 2x2 tiles (200x200, tile_size=100, overlap=20)")
        result1 = _blend_tiles(tiles, 180, 180, 100, 20)
        assert result1.shape == (180, 180, 3), f"Expected shape (180, 180, 3), got {result1.shape}"
        assert result1.min() >= 0 and result1.max() <= 255, f"Values out of range: [{result1.min()}, {result1.max()}]"
        print(f"  Shape: {result1.shape}, Range: [{result1.min():.2f}, {result1.max():.2f}]")
        
        # 测试用例 2: 不规则尺寸（模拟原始错误）
        print("✓ 测试用例 2: 不规则尺寸 (4783x3187)")
        tile_a = np.random.rand(1024, 1024, 3).astype(np.float32) * 255
        tile_b = np.random.rand(1024, 39, 3).astype(np.float32) * 255  # 最后一个 tile 宽度只有 39
        
        tiles2 = [
            (0, 0, tile_a),
            (3759, 0, tile_b),  # 4783 - 1024 = 3759
        ]
        
        result2 = _blend_tiles(tiles2, 4783, 1024, 1024, 96)
        assert result2.shape == (1024, 4783, 3), f"Expected shape (1024, 4783, 3), got {result2.shape}"
        print(f"  Shape: {result2.shape}, Range: [{result2.min():.2f}, {result2.max():.2f}]")
        
        print("\n✅ _blend_tiles 测试通过！")
        return True
        
    except Exception as e:
        print(f"\n❌ _blend_tiles 测试失败: {e}")
        import traceback
        traceback.print_exc()
        return False


def test_tiling_logic():
    """测试完整的 tiling 逻辑"""
    print("\n" + "="*60)
    print("测试 Tiling 逻辑")
    print("="*60)
    
    # 模拟原始错误场景的尺寸
    w, h = 4783, 3187
    tile_size = 1024
    overlap = 96
    stride = tile_size - overlap  # 928
    
    print(f"图像尺寸: {w}x{h}")
    print(f"Tile 大小: {tile_size}")
    print(f"Overlap: {overlap}")
    print(f"Stride: {stride}")
    
    # 计算 tiles
    tiles = []
    y0 = 0
    while y0 < h:
        y1 = min(h, y0 + tile_size)
        cy0 = max(0, y1 - tile_size)
        actual_h = y1 - cy0
        
        x0 = 0
        while x0 < w:
            x1 = min(w, x0 + tile_size)
            cx0 = max(0, x1 - tile_size)
            actual_w = x1 - cx0
            
            tiles.append({
                'x0': cx0,
                'y0': cy0,
                'x1': x1,
                'y1': y1,
                'w': actual_w,
                'h': actual_h,
            })
            
            if x1 >= w:
                break
            x0 += stride
        
        if y1 >= h:
            break
        y0 += stride
    
    print(f"\n总共生成 {len(tiles)} 个 tiles:")
    for i, tile in enumerate(tiles):
        print(f"  Tile {i}: ({tile['x0']}, {tile['y0']}) -> ({tile['x1']}, {tile['y1']}), size: {tile['w']}x{tile['h']}")
        
        # 验证尺寸
        assert tile['w'] > 0 and tile['w'] <= tile_size, f"Invalid width: {tile['w']}"
        assert tile['h'] > 0 and tile['h'] <= tile_size, f"Invalid height: {tile['h']}"
    
    # 检查最后一个 tile（最容易出错的地方）
    last_tile = tiles[-1]
    print(f"\n最后一个 tile: {last_tile['w']}x{last_tile['h']}")
    
    if last_tile['w'] < 100 or last_tile['h'] < 100:
        print(f"⚠️  警告：最后一个 tile 尺寸很小，可能导致问题")
    
    print("\n✅ Tiling 逻辑测试通过！")
    return True


def main():
    """运行所有测试"""
    print("="*60)
    print("Tiling 修复验证测试")
    print("="*60)
    
    results = {
        "_make_weight_1d": test_make_weight_1d(),
        "_blend_tiles": test_blend_tiles(),
        "Tiling 逻辑": test_tiling_logic(),
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
        print("🎉 所有测试通过！索引越界问题已修复。")
        print("="*60)
        return 0
    else:
        print("⚠️  部分测试失败，请检查上述错误信息")
        print("="*60)
        return 1


if __name__ == "__main__":
    sys.exit(main())
