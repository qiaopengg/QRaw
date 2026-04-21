#!/usr/bin/env python3
"""
测试进度条样式

展示新的进度条格式：迁移进度：=========== (1%)
"""
import time


def print_progress(percentage: int, description: str) -> None:
    """
    打印带进度条的进度信息
    格式：迁移进度：=========== (1%)
    """
    # 计算进度条长度（总共50个字符）
    bar_length = 50
    filled_length = int(bar_length * percentage / 100)
    bar = '=' * filled_length + ' ' * (bar_length - filled_length)
    
    # 输出格式化的进度条
    print(f"[PROGRESS] 迁移进度：{bar} ({percentage}%) - {description}")


def demo_single_image():
    """演示单图处理进度"""
    print("\n" + "=" * 80)
    print("演示 1: 单图处理进度")
    print("=" * 80 + "\n")
    
    stages = [
        (0, "开始风格迁移..."),
        (5, "加载图像..."),
        (10, "准备控制图像..."),
        (20, "运行 SDXL Base Model (步数: 35)..."),
        (70, "运行 SDXL Refiner..."),
        (90, "Refiner 完成"),
        (92, "应用色彩对齐..."),
        (94, "应用 RAW 融合..."),
        (96, "保存输出文件..."),
        (100, "风格迁移完成！"),
    ]
    
    for pct, desc in stages:
        print_progress(pct, desc)
        time.sleep(0.3)


def demo_tiled_processing():
    """演示分块处理进度"""
    print("\n" + "=" * 80)
    print("演示 2: 分块处理进度 (24 tiles)")
    print("=" * 80 + "\n")
    
    print_progress(0, "开始风格迁移...")
    time.sleep(0.2)
    print_progress(5, "加载图像...")
    time.sleep(0.2)
    
    print("[PROGRESS] 开始分块处理：图像 4783x3187，分为 24 个块...")
    time.sleep(0.2)
    
    total_tiles = 24
    for tile_idx in range(total_tiles):
        progress_pct = int((tile_idx / total_tiles) * 80) + 10
        x = (tile_idx % 6) * 1024
        y = (tile_idx // 6) * 1024
        print_progress(progress_pct, f"处理第 {tile_idx + 1}/{total_tiles} 块 (位置: {x},{y})")
        time.sleep(0.15)
    
    print_progress(90, f"融合 {total_tiles} 个块...")
    time.sleep(0.2)
    print_progress(92, "应用色彩对齐...")
    time.sleep(0.2)
    print_progress(94, "应用 RAW 融合...")
    time.sleep(0.2)
    print_progress(96, "保存输出文件...")
    time.sleep(0.2)
    print_progress(100, "风格迁移完成！")


def demo_progress_bar_styles():
    """演示不同百分比的进度条样式"""
    print("\n" + "=" * 80)
    print("演示 3: 进度条样式展示")
    print("=" * 80 + "\n")
    
    for pct in [0, 10, 25, 50, 75, 90, 100]:
        print_progress(pct, f"当前进度 {pct}%")
        time.sleep(0.3)


def main():
    """运行所有演示"""
    print("\n" + "=" * 80)
    print("风格迁移进度条样式测试")
    print("=" * 80)
    
    try:
        demo_single_image()
        time.sleep(1)
        
        demo_tiled_processing()
        time.sleep(1)
        
        demo_progress_bar_styles()
        
        print("\n" + "=" * 80)
        print("✅ 演示完成！")
        print("=" * 80 + "\n")
        
    except KeyboardInterrupt:
        print("\n\n⚠️  演示被中断")


if __name__ == "__main__":
    main()
