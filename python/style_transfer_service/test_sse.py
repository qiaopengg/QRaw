#!/usr/bin/env python3
"""
测试 SSE 进度端点
"""
import requests
import json
import time


def test_sse_endpoint():
    """测试 SSE 端点"""
    task_id = "test-task-123"
    url = f"http://127.0.0.1:7860/v1/style-transfer/progress/{task_id}"
    
    print(f"连接到 SSE 端点: {url}")
    print("=" * 60)
    
    try:
        response = requests.get(url, stream=True, timeout=30)
        
        if response.status_code != 200:
            print(f"错误: HTTP {response.status_code}")
            return
        
        print("✅ 连接成功，等待进度数据...")
        print("=" * 60)
        
        for line in response.iter_lines():
            if line:
                decoded_line = line.decode('utf-8')
                
                # 跳过心跳
                if decoded_line.startswith(':'):
                    print(f"💓 心跳: {decoded_line}")
                    continue
                
                # 解析数据
                if decoded_line.startswith('data:'):
                    data_str = decoded_line[5:].strip()
                    try:
                        data = json.loads(data_str)
                        
                        if data.get('type') == 'done':
                            print(f"\n✅ 完成: {data}")
                            break
                        elif data.get('type') == 'error':
                            print(f"\n❌ 错误: {data}")
                            break
                        else:
                            # 进度数据
                            pct = data.get('percentage', 0)
                            desc = data.get('description', '')
                            bar = data.get('bar', '')
                            print(f"📊 {pct}% - {desc}")
                            if bar:
                                print(f"    {bar}")
                    except json.JSONDecodeError as e:
                        print(f"⚠️  JSON 解析错误: {e}")
                        print(f"    原始数据: {data_str}")
    
    except requests.exceptions.Timeout:
        print("⏱️  超时")
    except requests.exceptions.ConnectionError:
        print("❌ 连接失败 - 请确保服务正在运行")
    except KeyboardInterrupt:
        print("\n\n⚠️  测试被中断")
    except Exception as e:
        print(f"❌ 错误: {e}")


if __name__ == "__main__":
    print("\n" + "=" * 60)
    print("SSE 进度端点测试")
    print("=" * 60 + "\n")
    
    print("注意: 此测试只验证 SSE 端点是否可访问")
    print("      实际进度数据需要通过风格迁移任务生成")
    print()
    
    test_sse_endpoint()
    
    print("\n" + "=" * 60)
    print("测试完成")
    print("=" * 60 + "\n")
