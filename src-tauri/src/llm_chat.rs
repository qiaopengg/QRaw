use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::Emitter;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AdjustmentSuggestion {
    pub key: String,
    pub value: f64,
    pub label: String,
    pub min: f64,
    pub max: f64,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatAdjustResponse {
    pub understanding: String,
    pub adjustments: Vec<AdjustmentSuggestion>,
}

/// 流式推送的事件载荷
#[derive(Serialize, Clone, Debug)]
pub struct StreamChunkPayload {
    /// "thinking" | "content" | "done" | "error"
    pub chunk_type: String,
    /// 当前 token 文本（thinking/content 时有值）
    pub text: String,
    /// 最终解析结果（done 时有值）
    pub result: Option<ChatAdjustResponse>,
}

fn build_system_prompt(current_adjustments: &Value) -> String {
    let adj_str = serde_json::to_string_pretty(current_adjustments).unwrap_or_default();

    format!(
        r#"你是一位专业摄影师助手，帮助用户通过自然语言调整照片参数。

## 你的任务
根据用户的描述，推断出最合适的调整参数，以 JSON 格式返回。

## 核心规则
1. value 是最终绝对值，不是增量。例如用户说"提高曝光"，你应该基于当前值计算出新的绝对值。
2. 必须参考"当前图像参数"，基于当前值做调整。用户可能已经手动拖过滑块。
3. 如果用户说"再亮一点"，在当前 exposure 基础上增加，而不是从 0 开始。
4. 如果用户说"太过了"或"回退一点"，在当前值基础上反向微调。

## 参数范围说明（步长）
- exposure（曝光）: -5.0 ~ 5.0，步长 0.01，0为原始
- brightness（亮度）: -100 ~ 100，步长 1，0为原始
- contrast（对比度）: -100 ~ 100，步长 1，0为原始
- highlights（高光）: -100 ~ 100，步长 1，0为原始
- shadows（阴影）: -100 ~ 100，步长 1，0为原始
- whites（白色）: -100 ~ 100，步长 1，0为原始
- blacks（黑色）: -100 ~ 100，步长 1，0为原始
- saturation（饱和度）: -100 ~ 100，步长 1，0为原始
- vibrance（自然饱和度）: -100 ~ 100，步长 1，0为原始
- temperature（色温）: -100 ~ 100，步长 1，负值偏冷/蓝，正值偏暖/橙
- tint（色调偏移）: -100 ~ 100，步长 1，负值偏绿，正值偏品红
- clarity（清晰度）: -100 ~ 100，步长 1，0为原始
- dehaze（去雾）: -100 ~ 100，步长 1，0为原始
- structure（结构）: -100 ~ 100，步长 1，0为原始
- sharpness（锐度）: 0 ~ 100，步长 1，0为原始
- vignetteAmount（暗角）: -100 ~ 100，步长 1，负值暗角，正值亮角

## 摄影语义映射规则
- "太暗/欠曝" → exposure +0.5~1.5, shadows +20~40
- "太亮/过曝" → exposure -0.5~-1.5, highlights -20~-40
- "阳光明媚/明亮" → exposure +0.5~1.0, highlights +10~20, temperature +10~20
- "青春活力/鲜艳" → saturation +20~40, vibrance +20~30, clarity +10~20
- "复古/胶片" → saturation -10~-20, temperature +10~20, contrast +10~20, vignetteAmount -20~-30
- "清新/自然" → saturation +5~15, temperature -5~-15, clarity +5~15
- "暗调/低沉" → exposure -0.5~-1.0, shadows -20~-30, contrast +10~20
- "高对比/戏剧" → contrast +30~50, highlights -20~-30, shadows -20~-30
- "柔和/梦幻" → clarity -10~-20, contrast -10~-20, highlights +10~20
- "冷色调/蓝调" → temperature -20~-40, tint -5~-15
- "暖色调/金色" → temperature +20~40, tint +5~10
- "去雾/通透" → dehaze +20~40, clarity +10~20
- "人像/皮肤" → saturation +5~10, temperature +5~10, clarity -5~-10
- "黑白/单色" → saturation -100, vibrance -100
- "电影感/cinematic" → contrast +15~25, temperature +5~15, vignetteAmount -15~-25, saturation -5~-15
- "再...一点" → 在当前值基础上小幅增量调整（±5~15）
- "太过了/回退" → 在当前值基础上反向调整（回退 30%~50%）

## 当前图像参数
```json
{adj_str}
```

## 输出格式（严格 JSON，禁止输出任何其他文字）
{{
  "understanding": "用一句话描述你对用户意图的理解",
  "adjustments": [
    {{
      "key": "参数键名",
      "value": 数值,
      "label": "中文参数名",
      "min": 最小值,
      "max": 最大值,
      "reason": "调整原因（简短）"
    }}
  ]
}}

只返回需要调整的参数（与当前值不同的），最多返回8个最重要的参数。

## 安全约束（必须遵守）
- exposure 绝对值不得超过 2.5（即 -2.5 ~ 2.5）
- 其他参数绝对值不得超过 80（即 -80 ~ 80）
- 用户说"太亮/太暗/太过了"时，只做小幅调整（exposure ±0.3~0.8，其他参数 ±10~30）
- 绝对禁止把 exposure 设为 3.0 以上或 -3.0 以下
- 如果用户的描述模糊，宁可保守调整也不要激进"#,
        adj_str = adj_str
    )
}


/// 剥离 Qwen3 模型的 <think>...</think> 思考标签
pub fn strip_thinking_tags(text: &str) -> String {
    let mut result = text.to_string();
    while let Some(start) = result.find("<think>") {
        if let Some(end) = result.find("</think>") {
            let end_pos = end + "</think>".len();
            result = format!("{}{}", &result[..start], &result[end_pos..]);
        } else {
            result = result[..start].to_string();
            break;
        }
    }
    result.trim().to_string()
}

/// 从文本中提取第一个完整的 JSON 对象
pub fn extract_json(text: &str) -> Result<String, String> {
    let cleaned = strip_thinking_tags(text);

    if serde_json::from_str::<Value>(&cleaned).is_ok() {
        return Ok(cleaned);
    }

    if let Some(start) = cleaned.find("```json") {
        let after_marker = &cleaned[start + 7..];
        if let Some(end) = after_marker.find("```") {
            let candidate = after_marker[..end].trim();
            if serde_json::from_str::<Value>(candidate).is_ok() {
                return Ok(candidate.to_string());
            }
        }
    }
    if let Some(start) = cleaned.find("```\n") {
        let after_marker = &cleaned[start + 4..];
        if let Some(end) = after_marker.find("```") {
            let candidate = after_marker[..end].trim();
            if serde_json::from_str::<Value>(candidate).is_ok() {
                return Ok(candidate.to_string());
            }
        }
    }

    if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
        if start < end {
            let candidate = &cleaned[start..=end];
            if serde_json::from_str::<Value>(candidate).is_ok() {
                return Ok(candidate.to_string());
            }
        }
    }

    Err(format!(
        "无法从响应中提取 JSON: {}",
        &cleaned[..cleaned.len().min(200)]
    ))
}

/// 流式 chat_adjust：通过 Tauri event 逐 token 推送，前端实时显示思考过程
#[tauri::command]
pub async fn chat_adjust(
    message: String,
    history: Vec<ChatMessage>,
    current_adjustments: Value,
    llm_endpoint: String,
    llm_api_key: Option<String>,
    llm_model: Option<String>,
    app_handle: tauri::AppHandle,
) -> Result<ChatAdjustResponse, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;

    let model = llm_model.unwrap_or_else(|| "qwen3.5:9b".to_string());
    let system_prompt = build_system_prompt(&current_adjustments);

    let mut messages: Vec<Value> = vec![json!({
        "role": "system",
        "content": system_prompt
    })];

    let recent_history: Vec<&ChatMessage> = history
        .iter()
        .rev()
        .take(8)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    for msg in recent_history {
        messages.push(json!({
            "role": msg.role,
            "content": msg.content
        }));
    }

    messages.push(json!({
        "role": "user",
        "content": message
    }));

    let endpoint = llm_endpoint.trim_end_matches('/').to_string();
    let url = format!("{}/v1/chat/completions", endpoint);

    // 流式请求
    let request_body = json!({
        "model": model,
        "messages": messages,
        "temperature": 0.3,
        "stream": true
    });

    let mut req = client.post(&url).json(&request_body);
    if let Some(key) = &llm_api_key {
        if !key.is_empty() {
            req = req.bearer_auth(key);
        }
    }

    let response = req.send().await.map_err(|e| format!("请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("LLM 返回错误 {}: {}", status, body));
    }

    // 逐行读取 SSE 流
    let mut full_content = String::new();
    let mut in_thinking = false;
    let mut thinking_buffer = String::new();

    use futures_util::StreamExt;
    let mut stream = response.bytes_stream();

    let mut line_buffer = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| format!("流读取错误: {}", e))?;
        let chunk_str = String::from_utf8_lossy(&chunk);
        line_buffer.push_str(&chunk_str);

        // SSE 格式：每行 "data: {...}\n\n"
        while let Some(newline_pos) = line_buffer.find('\n') {
            let line = line_buffer[..newline_pos].trim().to_string();
            line_buffer = line_buffer[newline_pos + 1..].to_string();

            if line.is_empty() || line == "data: [DONE]" {
                continue;
            }

            if let Some(json_str) = line.strip_prefix("data: ") {
                if let Ok(sse_json) = serde_json::from_str::<Value>(json_str) {
                    if let Some(delta_content) = sse_json["choices"][0]["delta"]["content"].as_str() {
                        if delta_content.is_empty() {
                            continue;
                        }

                        full_content.push_str(delta_content);

                        // 检测 <think> 标签状态
                        if delta_content.contains("<think>") {
                            in_thinking = true;
                        }

                        if in_thinking {
                            thinking_buffer.push_str(delta_content);
                            // 清理标签后推送思考内容
                            let clean = delta_content.replace("<think>", "").replace("</think>", "");
                            if !clean.is_empty() {
                                let _ = app_handle.emit("chat-stream-chunk", StreamChunkPayload {
                                    chunk_type: "thinking".to_string(),
                                    text: clean,
                                    result: None,
                                });
                            }
                        }

                        if delta_content.contains("</think>") {
                            in_thinking = false;
                            thinking_buffer.clear();
                            // 思考结束，推送过渡提示
                            let _ = app_handle.emit("chat-stream-chunk", StreamChunkPayload {
                                chunk_type: "thinking".to_string(),
                                text: "\n正在生成调整参数...\n".to_string(),
                                result: None,
                            });
                        }

                        // 非思考内容是 JSON 格式，不推送给前端显示
                        // 前端会在 done 事件中获取解析后的自然语言 understanding
                    }
                }
            }
        }
    }

    // 流结束，解析完整内容
    let json_str = extract_json(&full_content)?;
    let mut parsed: ChatAdjustResponse = serde_json::from_str(&json_str)
        .map_err(|e| format!("解析 JSON 失败: {}，原始内容: {}", e, &full_content[..full_content.len().min(500)]))?;

    // 安全钳位：防止 LLM 返回极端值导致白图/黑图
    for adj in &mut parsed.adjustments {
        match adj.key.as_str() {
            "exposure" => adj.value = adj.value.max(-2.5).min(2.5),
            _ => adj.value = adj.value.max(-80.0).min(80.0),
        }
    }

    // 推送完成事件
    let _ = app_handle.emit("chat-stream-chunk", StreamChunkPayload {
        chunk_type: "done".to_string(),
        text: String::new(),
        result: Some(parsed.clone()),
    });

    Ok(parsed)
}
