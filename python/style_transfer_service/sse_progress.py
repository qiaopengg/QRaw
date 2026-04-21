"""
SSE 进度推送模块

提供实时进度更新功能
"""
import json
import time
from queue import Queue, Empty
from typing import Generator


def progress_stream(task_id: str, progress_queue: Queue, timeout: int = 3600) -> Generator[str, None, None]:
    """
    SSE 进度流生成器
    
    Args:
        task_id: 任务ID
        progress_queue: 进度队列
        timeout: 超时时间（秒）
    
    Yields:
        SSE 格式的进度数据
    """
    start_time = time.time()
    
    while True:
        # 检查超时
        if time.time() - start_time > timeout:
            yield f"data: {json.dumps({'type': 'error', 'message': 'timeout'})}\n\n"
            break
        
        try:
            # 从队列获取进度数据（阻塞1秒）
            progress_data = progress_queue.get(timeout=1)
            
            # 检查是否是结束信号
            if progress_data.get("type") == "done":
                yield f"data: {json.dumps(progress_data)}\n\n"
                break
            
            # 发送进度数据
            yield f"data: {json.dumps(progress_data)}\n\n"
            
            # 如果进度达到100%，等待done信号或超时
            if progress_data.get("percentage") == 100:
                try:
                    final_data = progress_queue.get(timeout=5)
                    yield f"data: {json.dumps(final_data)}\n\n"
                except Empty:
                    pass
                break
                
        except Empty:
            # 队列为空，发送心跳
            yield f": heartbeat\n\n"
            continue
        except Exception as e:
            yield f"data: {json.dumps({'type': 'error', 'message': str(e)})}\n\n"
            break
