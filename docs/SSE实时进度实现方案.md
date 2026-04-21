# SSE 实时进度实现方案

## 📋 概述

由于当前架构使用 HTTP POST 同步请求，无法实时传输进度。我们需要实现 **Server-Sent Events (SSE)** 来支持实时进度推送。

## 🏗️ 架构设计

```
┌─────────────────────────────────────────────────────────────┐
│                      前端 (ChatPanel.tsx)                    │
│  - 监听 'style-transfer-stream' 事件                         │
│  - 显示实时进度在 ThinkingBlock                              │
└─────────────────────────────────────────────────────────────┘
                              ↑
                              │ Tauri Event
                              │
┌─────────────────────────────────────────────────────────────┐
│              Rust 后端 (style_transfer_runtime.rs)           │
│  - 生成唯一 task_id                                          │
│  - 启动 SSE 监听线程                                         │
│  - 发起 HTTP POST 请求（异步）                               │
│  - 转发 SSE 进度到前端                                       │
└─────────────────────────────────────────────────────────────┘
                    ↑                    ↑
                    │ SSE Stream         │ HTTP POST
                    │                    │
┌─────────────────────────────────────────────────────────────┐
│           Python 服务 (app.py)                               │
│  - POST /v1/style-transfer (接收任务)                        │
│  - GET /v1/style-transfer/progress/{task_id} (SSE 端点)     │
│  - 进度队列管理                                              │
└─────────────────────────────────────────────────────────────┘
```

## ✅ 已完成的工作

### 1. Python 服务端

#### 添加的功能：

- ✅ 进度队列管理系统
- ✅ `_emit_progress()` 函数 - 发送进度到队列
- ✅ SSE 端点 `/v1/style-transfer/progress/{task_id}`
- ✅ `task_id` 参数支持
- ✅ `sse_progress.py` 模块

#### 修改的文件：

- `python/style_transfer_service/app.py`
- `python/style_transfer_service/sse_progress.py` (新建)

### 2. 测试脚本

- ✅ `test_sse.py` - SSE 端点测试

## 🔧 待完成的工作

### 1. 修改所有进度调用点

需要将 `task_id` 传递到所有调用 `_print_progress()` 的地方：

- `_run_single()` 函数
- `_run_tiled()` 函数
- 所有后处理步骤

### 2. Rust 后端实现

需要修改 `src-tauri/src/style_transfer_runtime.rs`：

```rust
async fn invoke_python_style_transfer_with_progress(
    request: &StyleTransferRunRequest,
    service_url: &str,
    preset: StyleTransferPreset,
    enable_refiner: bool,
    app_handle: &tauri::AppHandle,
) -> Result<StyleTransferExecutionResponse, String> {
    // 1. 生成唯一 task_id
    let task_id = uuid::Uuid::new_v4().to_string();

    // 2. 启动 SSE 监听线程
    let sse_url = format!("{}/v1/style-transfer/progress/{}", service_url, task_id);
    let app_handle_clone = app_handle.clone();

    tokio::spawn(async move {
        // 连接 SSE 端点
        let client = reqwest::Client::new();
        let mut stream = client.get(&sse_url).send().await?.bytes_stream();

        // 监听进度事件
        while let Some(chunk) = stream.next().await {
            let data = chunk?;
            let text = String::from_utf8_lossy(&data);

            // 解析 SSE 数据
            for line in text.lines() {
                if line.starts_with("data:") {
                    let json_str = &line[5..].trim();
                    if let Ok(progress) = serde_json::from_str::<Value>(json_str) {
                        // 转发到前端
                        emit_style_transfer_status(&app_handle_clone, progress["message"].as_str().unwrap_or(""));
                    }
                }
            }
        }
    });

    // 3. 发起 HTTP POST 请求（带 task_id）
    let payload = PythonStyleTransferRequest {
        // ... 其他字段
        task_id: Some(task_id.clone()),
    };

    // ... 其余代码
}
```

### 3. 前端（无需修改）

前端已经准备好接收进度，无需修改。

## 📝 实现步骤

### 步骤 1: 完成 Python 端修改

```bash
cd python/style_transfer_service

# 修改所有进度调用点，传入 task_id
# 这需要修改以下函数：
# - _run_single(req, task_id)
# - _run_tiled(req, task_id)
# - style_transfer() 中的所有 _print_progress 调用
```

### 步骤 2: 测试 SSE 端点

```bash
# 启动服务
python3 app.py

# 在另一个终端测试
python3 test_sse.py
```

### 步骤 3: 实现 Rust 端

```bash
# 添加依赖到 Cargo.toml
[dependencies]
uuid = { version = "1.0", features = ["v4"] }
futures = "0.3"

# 修改 src-tauri/src/style_transfer_runtime.rs
# 实现 SSE 监听和转发逻辑
```

### 步骤 4: 集成测试

1. 启动 Python 服务
2. 启动 RapidRAW 应用
3. 执行风格迁移
4. 观察进度条是否实时更新

## 🎯 预期效果

完成后，用户将看到：

```
🧠 思考中... [📋]
├─ [PROGRESS] 迁移进度：                                                   (0%) - 开始风格迁移...
├─ [PROGRESS] 迁移进度：==                                                 (5%) - 加载图像...
├─ [PROGRESS] 迁移进度：=====                                              (10%) - 处理第 1/24 块...
├─ [PROGRESS] 迁移进度：=========================                          (50%) - 处理第 13/24 块...
└─ [PROGRESS] 迁移进度：================================================== (100%) - 完成！
```

## ⚠️ 注意事项

1. **性能影响**：SSE 连接会占用一个 HTTP 连接，但影响很小
2. **超时处理**：SSE 连接设置 1 小时超时
3. **错误处理**：需要处理 SSE 连接断开的情况
4. **队列清理**：任务完成后需要清理进度队列

## 🔍 调试方法

### Python 端调试

```bash
# 启用调试模式
export QRAW_DEBUG="1"
python3 app.py

# 查看进度输出
tail -f /path/to/service.log
```

### Rust 端调试

```rust
// 添加日志
println!("SSE 连接: {}", sse_url);
println!("收到进度: {:?}", progress);
```

### 前端调试

```typescript
// 在 ChatPanel.tsx 中添加
console.log('收到进度事件:', event.payload);
```

## 📚 相关文档

- [Server-Sent Events (MDN)](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events)
- [FastAPI StreamingResponse](https://fastapi.tiangolo.com/advanced/custom-response/#streamingresponse)
- [Tauri Event System](https://tauri.app/v1/guides/features/events/)

---

**状态：** 🟡 部分完成（Python 端已实现，Rust 端待实现）

**下一步：** 实现 Rust 端 SSE 监听和转发逻辑

**预计工作量：** 2-3 小时
