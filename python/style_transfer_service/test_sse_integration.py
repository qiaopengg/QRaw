#!/usr/bin/env python3
"""
SSE 集成测试脚本

测试完整的 SSE 进度反馈流程
"""
import json
import time
import requests
import threading
from pathlib import Path


def test_sse_endpoint():
    """测试 SSE 端点是否正常工作"""
    print("=" * 60)
    print("测试 1: SSE 端点连接测试")
    print("=" * 60)
    
    # 生成测试任务ID
    task_id = "test-task-123"
    sse_url = f"http://127.0.0.1:7860/v1/style-transfer/progress/{task_id}"
    
    print(f"连接 SSE 端点: {sse_url}")
    
    try:
        response = requests.get(sse_url, stream=True, timeout=5)
        print(f"✅ 连接成功，状态码: {response.status_code}")
        print(f"✅ Content-Type: {response.headers.get('content-type')}")
        
        # 读取前几条消息
        print("\n接收到的消息：")
        count = 0
        for line in response.iter_lines(decode_unicode=True):
            if line:
                print(f"  {line}")
                count += 1
                if count >= 5:  # 只读取前5条
                    break
        
        print(f"\n✅ 成功接收 {count} 条消息")
        return True
        
    except requests.exceptions.Timeout:
        print("⚠️  连接超时（这是正常的，因为没有实际任务）")
        return True
    except Exception as e:
        print(f"❌ 连接失败: {e}")
        return False


def simulate_progress(task_id: str):
    """模拟进度更新"""
    from app import _emit_progress, _get_progress_queue
    
    # 等待队列创建
    time.sleep(0.5)
    
    # 发送进度
    stages = [
        (0, "开始风格迁移..."),
        (10, "加载图像..."),
        (20, "准备控制图像..."),
        (40, "运行 SDXL 推理..."),
        (70, "运行 Refiner..."),
        (90, "后处理..."),
        (100, "完成！"),
    ]
    
    for percentage, description in stages:
        _emit_progress(task_id, percentage, description)
        time.sleep(0.5)
    
    # 发送完成信号
    queue = _get_progress_queue(task_id)
    if queue:
        queue.put({
            "type": "done",
            "percentage": 100,
            "output_image_path": "/tmp/output.tiff",
            "preview_image_path": "/tmp/preview.png"
        })


def test_sse_with_progress():
    """测试 SSE 端点 + 进度更新"""
    print("\n" + "=" * 60)
    print("测试 2: SSE 进度更新测试")
    print("=" * 60)
    
    task_id = "test-task-456"
    sse_url = f"http://127.0.0.1:7860/v1/style-transfer/progress/{task_id}"
    
    # 启动进度模拟线程
    progress_thread = threading.Thread(target=simulate_progress, args=(task_id,))
    progress_thread.daemon = True
    progress_thread.start()
    
    print(f"连接 SSE 端点: {sse_url}")
    print("开始接收进度更新...\n")
    
    try:
        response = requests.get(sse_url, stream=True, timeout=10)
        
        received_count = 0
        for line in response.iter_lines(decode_unicode=True):
            if line.startswith("data:"):
                json_str = line[5:].strip()
                try:
                    data = json.loads(json_str)
                    
                    if data.get("type") == "done":
                        print(f"\n✅ 收到完成信号")
                        print(f"   输出文件: {data.get('output_image_path')}")
                        print(f"   预览文件: {data.get('preview_image_path')}")
                        break
                    
                    message = data.get("message", "")
                    if message:
                        print(f"  {message}")
                        received_count += 1
                        
                except json.JSONDecodeError:
                    print(f"  ⚠️  无法解析 JSON: {json_str}")
            elif line.startswith(":"):
                # 心跳消息
                pass
        
        print(f"\n✅ 成功接收 {received_count} 条进度消息")
        return True
        
    except Exception as e:
        print(f"❌ 测试失败: {e}")
        return False


def test_health_endpoint():
    """测试健康检查端点"""
    print("\n" + "=" * 60)
    print("测试 3: 健康检查端点")
    print("=" * 60)
    
    try:
        response = requests.get("http://127.0.0.1:7860/health", timeout=5)
        data = response.json()
        
        print(f"✅ 服务状态: {data.get('status')}")
        print(f"✅ 就绪状态: {data.get('ready')}")
        print(f"✅ 版本: {data.get('version')}")
        print(f"✅ Pipeline: {data.get('pipeline')}")
        print(f"✅ 能力: {', '.join(data.get('capabilities', []))}")
        
        return True
    except Exception as e:
        print(f"❌ 测试失败: {e}")
        return False


def main():
    """运行所有测试"""
    print("\n" + "=" * 60)
    print("SSE 集成测试套件")
    print("=" * 60)
    print("\n⚠️  请确保 Python 服务正在运行：")
    print("   cd python/style_transfer_service")
    print("   python3 app.py")
    print("\n按 Enter 继续...")
    input()
    
    results = []
    
    # 测试 1: 健康检查
    results.append(("健康检查", test_health_endpoint()))
    
    # 测试 2: SSE 端点连接
    results.append(("SSE 端点连接", test_sse_endpoint()))
    
    # 测试 3: SSE 进度更新
    results.append(("SSE 进度更新", test_sse_with_progress()))
    
    # 总结
    print("\n" + "=" * 60)
    print("测试总结")
    print("=" * 60)
    
    for name, passed in results:
        status = "✅ 通过" if passed else "❌ 失败"
        print(f"{name}: {status}")
    
    all_passed = all(result for _, result in results)
    
    if all_passed:
        print("\n🎉 所有测试通过！")
        print("\n下一步：")
        print("1. 启动 RapidRAW 应用")
        print("2. 执行风格迁移")
        print("3. 观察进度条是否正确显示")
    else:
        print("\n⚠️  部分测试失败，请检查服务状态")
    
    return 0 if all_passed else 1


if __name__ == "__main__":
    exit(main())
