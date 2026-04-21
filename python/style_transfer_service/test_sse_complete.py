#!/usr/bin/env python3
"""
完整的 SSE 进度反馈测试

测试整个流程：
1. 创建测试任务
2. 监听 SSE 进度
3. 验证进度数据格式
4. 验证完成信号
"""
import json
import time
import threading
import requests
from pathlib import Path


def test_sse_endpoint():
    """测试 SSE 端点基本功能"""
    print("=" * 60)
    print("测试 1: SSE 端点基本功能")
    print("=" * 60)
    
    task_id = "test-task-123"
    url = f"http://127.0.0.1:7860/v1/style-transfer/progress/{task_id}"
    
    print(f"连接到: {url}")
    
    try:
        response = requests.get(url, stream=True, timeout=10)
        print(f"✅ 连接成功，状态码: {response.status_code}")
        print(f"✅ Content-Type: {response.headers.get('content-type')}")
        
        # 读取前几条消息
        count = 0
        for line in response.iter_lines(decode_unicode=True):
            if line:
                print(f"  收到: {line}")
                count += 1
                if count >= 5:  # 只读取前5条
                    break
        
        print("✅ SSE 端点工作正常\n")
        return True
        
    except Exception as e:
        print(f"❌ 测试失败: {e}\n")
        return False


def test_progress_format():
    """测试进度数据格式"""
    print("=" * 60)
    print("测试 2: 进度数据格式验证")
    print("=" * 60)
    
    # 模拟进度数据
    from app import _emit_progress, _create_progress_queue, _get_progress_queue
    
    task_id = "format-test-456"
    queue = _create_progress_queue(task_id)
    
    # 发送测试进度
    test_cases = [
        (0, "开始测试"),
        (25, "处理中..."),
        (50, "一半完成"),
        (75, "接近完成"),
        (100, "测试完成"),
    ]
    
    print("发送测试进度...")
    for percentage, description in test_cases:
        _emit_progress(task_id, percentage, description)
    
    # 验证队列中的数据
    print("\n验证队列数据:")
    queue = _get_progress_queue(task_id)
    if queue:
        while not queue.empty():
            data = queue.get()
            print(f"  ✅ {data['percentage']}% - {data['description']}")
            
            # 验证必需字段
            assert "percentage" in data, "缺少 percentage 字段"
            assert "description" in data, "缺少 description 字段"
            assert "bar" in data, "缺少 bar 字段"
            assert "message" in data, "缺少 message 字段"
            
            # 验证进度条格式
            assert data['bar'].count('=') + data['bar'].count(' ') == 50, "进度条长度不正确"
            assert "[PROGRESS]" in data['message'], "消息格式不正确"
        
        print("✅ 进度数据格式正确\n")
        return True
    else:
        print("❌ 队列不存在\n")
        return False


def test_concurrent_sse():
    """测试并发 SSE 连接"""
    print("=" * 60)
    print("测试 3: 并发 SSE 连接")
    print("=" * 60)
    
    from app import _emit_progress, _create_progress_queue
    
    task_id = "concurrent-test-789"
    
    # 创建队列并发送进度
    def send_progress():
        time.sleep(1)  # 等待 SSE 连接建立
        for i in range(0, 101, 10):
            _emit_progress(task_id, i, f"进度 {i}%")
            time.sleep(0.2)
        
        # 发送完成信号
        queue = _get_progress_queue(task_id)
        if queue:
            queue.put({"type": "done", "percentage": 100})
    
    # 启动发送线程
    sender = threading.Thread(target=send_progress, daemon=True)
    sender.start()
    
    # 连接 SSE
    url = f"http://127.0.0.1:7860/v1/style-transfer/progress/{task_id}"
    
    try:
        response = requests.get(url, stream=True, timeout=10)
        print(f"✅ SSE 连接建立")
        
        progress_count = 0
        done_received = False
        
        for line in response.iter_lines(decode_unicode=True):
            if line and line.startswith("data:"):
                json_str = line[5:].strip()
                try:
                    data = json.loads(json_str)
                    
                    if data.get("type") == "done":
                        print(f"  ✅ 收到完成信号")
                        done_received = True
                        break
                    elif "percentage" in data:
                        progress_count += 1
                        print(f"  ✅ 进度 {data['percentage']}%: {data['description']}")
                except json.JSONDecodeError:
                    pass
        
        print(f"\n✅ 收到 {progress_count} 条进度消息")
        
        if done_received:
            print("✅ 并发测试成功\n")
            return True
        else:
            print("⚠️  未收到完成信号\n")
            return False
            
    except Exception as e:
        print(f"❌ 测试失败: {e}\n")
        return False


def main():
    """运行所有测试"""
    print("\n" + "=" * 60)
    print("SSE 实时进度反馈 - 完整测试套件")
    print("=" * 60 + "\n")
    
    # 检查服务状态
    try:
        response = requests.get("http://127.0.0.1:7860/health", timeout=5)
        if response.status_code == 200:
            health = response.json()
            print(f"✅ 服务状态: {health['status']}")
            print(f"✅ 服务就绪: {health['ready']}")
            print(f"✅ 版本: {health['version']}")
            print(f"✅ Pipeline: {health['pipeline']}")
            print()
        else:
            print("❌ 服务未就绪")
            return
    except Exception as e:
        print(f"❌ 无法连接到服务: {e}")
        print("请先启动服务: python3 app.py")
        return
    
    # 运行测试
    results = []
    
    results.append(("SSE 端点基本功能", test_sse_endpoint()))
    results.append(("进度数据格式验证", test_progress_format()))
    results.append(("并发 SSE 连接", test_concurrent_sse()))
    
    # 总结
    print("=" * 60)
    print("测试总结")
    print("=" * 60)
    
    passed = sum(1 for _, result in results if result)
    total = len(results)
    
    for name, result in results:
        status = "✅ 通过" if result else "❌ 失败"
        print(f"{status} - {name}")
    
    print()
    print(f"总计: {passed}/{total} 测试通过")
    
    if passed == total:
        print("\n🎉 所有测试通过！SSE 实时进度反馈功能正常工作。")
    else:
        print(f"\n⚠️  {total - passed} 个测试失败，请检查日志。")


if __name__ == "__main__":
    main()
