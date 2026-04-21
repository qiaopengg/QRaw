#!/usr/bin/env python3
"""
诊断色彩丢失问题
"""
import numpy as np
from PIL import Image
import sys

def check_image_color(img_path: str):
    """检查图像是否有色彩"""
    try:
        img = Image.open(img_path)
        print(f"\n检查图像: {img_path}")
        print(f"  模式: {img.mode}")
        print(f"  尺寸: {img.size}")
        
        arr = np.array(img)
        print(f"  数组形状: {arr.shape}")
        print(f"  数据类型: {arr.dtype}")
        print(f"  值域: [{arr.min()}, {arr.max()}]")
        
        if len(arr.shape) == 3 and arr.shape[2] == 3:
            # RGB 图像
            r_mean = arr[:,:,0].mean()
            g_mean = arr[:,:,1].mean()
            b_mean = arr[:,:,2].mean()
            
            print(f"  R 通道平均值: {r_mean:.2f}")
            print(f"  G 通道平均值: {g_mean:.2f}")
            print(f"  B 通道平均值: {b_mean:.2f}")
            
            # 检查是否是灰度图（三个通道值相同）
            r_std = arr[:,:,0].std()
            g_std = arr[:,:,1].std()
            b_std = arr[:,:,2].std()
            
            channel_diff = np.abs(arr[:,:,0].astype(float) - arr[:,:,1]) + \
                          np.abs(arr[:,:,1].astype(float) - arr[:,:,2]) + \
                          np.abs(arr[:,:,0].astype(float) - arr[:,:,2])
            
            avg_channel_diff = channel_diff.mean()
            
            print(f"  通道差异平均值: {avg_channel_diff:.2f}")
            
            if avg_channel_diff < 1.0:
                print(f"  ⚠️  警告：图像可能是灰度图（三个通道几乎相同）")
                print(f"  ❌ 色彩丢失！")
                return False
            else:
                print(f"  ✅ 图像有色彩")
                return True
        elif len(arr.shape) == 2:
            print(f"  ❌ 这是一个灰度图像（单通道）")
            return False
        else:
            print(f"  ⚠️  未知图像格式")
            return False
            
    except Exception as e:
        print(f"  ❌ 错误: {e}")
        return False


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("用法: python3 diagnose_color_loss.py <图像路径>")
        print("\n或者检查最近的输出:")
        
        import os
        import tempfile
        from pathlib import Path
        
        output_dir = Path(tempfile.gettempdir()) / "qraw-style-transfer"
        if output_dir.exists():
            # 找到最新的输出
            subdirs = [d for d in output_dir.iterdir() if d.is_dir()]
            if subdirs:
                latest = max(subdirs, key=lambda d: d.stat().st_mtime)
                print(f"\n最新输出目录: {latest}")
                
                output_tiff = latest / "output.tiff"
                preview_png = latest / "preview.png"
                
                if output_tiff.exists():
                    check_image_color(str(output_tiff))
                
                if preview_png.exists():
                    check_image_color(str(preview_png))
            else:
                print("没有找到输出文件")
        else:
            print(f"输出目录不存在: {output_dir}")
    else:
        for img_path in sys.argv[1:]:
            check_image_color(img_path)
