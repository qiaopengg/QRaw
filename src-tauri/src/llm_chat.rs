use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AdjustmentSuggestion {
    pub key: String,
    pub value: f64,
    pub label: String,
    pub min: f64,
    pub max: f64,
    pub reason: String,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ChatAdjustResponse {
    pub understanding: String,
    pub adjustments: Vec<AdjustmentSuggestion>,
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
- "太亮/过曝" → exposure -0.5~1.5, highlights -20~40
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

只返回需要调整的参数（与当前值不同的），最多返回8个最重要的参数。"#,
        adj_str = adj_str
    )
}


/// 剥离 Qwen3 模型的 <think>...</think> 思考标签
fn strip_thinking_tags(text: &str) -> String {
    let mut result = text.to_string();
    // 循环移除所有 <think>...</think> 块（包括嵌套情况）
    while let Some(start) = result.find("<think>") {
        if let Some(end) = result.find("</think>") {
            let end_pos = end + "</think>".len();
            result = format!("{}{}", &result[..start], &result[end_pos..]);
        } else {
            // 有 <think> 但没有 </think>，截断到 <think> 之前
            result = result[..start].to_string();
            break;
        }
    }
    result.trim().to_string()
}

/// 从文本中提取第一个完整的 JSON 对象
fn extract_json(text: &str) -> Result<String, String> {
    // 先剥离思考标签
    let cleaned = strip_thinking_tags(text);

    // 先尝试直接解析
    if serde_json::from_str::<Value>(&cleaned).is_ok() {
        return Ok(cleaned);
    }

    // 尝试从 markdown 代码块中提取
    if let Some(start) = cleaned.find("```json") {
        let after_marker = &cleaned[start + 7..];
        if let Some(end) = after_marker.find("```") {
            let candidate = after_marker[..end].trim();
            if serde_json::from_str::<Value>(candidate).is_ok() {
                return Ok(candidate.to_string());
            }
        }
    }
    // 通用代码块
    if let Some(start) = cleaned.find("```\n") {
        let after_marker = &cleaned[start + 4..];
        if let Some(end) = after_marker.find("```") {
            let candidate = after_marker[..end].trim();
            if serde_json::from_str::<Value>(candidate).is_ok() {
                return Ok(candidate.to_string());
            }
        }
    }

    // 找到第一个 { 和最后一个 }
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

#[tauri::command]
pub async fn chat_adjust(
    message: String,
    history: Vec<ChatMessage>,
    current_adjustments: Value,
    llm_endpoint: String,
    llm_api_key: Option<String>,
    llm_model: Option<String>,
) -> Result<ChatAdjustResponse, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| e.to_string())?;

    let model = llm_model.unwrap_or_else(|| "qwen3.5:9b".to_string());
    let system_prompt = build_system_prompt(&current_adjustments);

    // 构建消息列表
    let mut messages: Vec<Value> = vec![json!({
        "role": "system",
        "content": system_prompt
    })];

    // 加入历史（最多保留最近8条）
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

    let request_body = json!({
        "model": model,
        "messages": messages,
        "temperature": 0.3,
        "stream": false,
        "response_format": {"type": "json_object"}
    });

    let mut req = client.post(&url).json(&request_body);

    if let Some(key) = &llm_api_key {
        if !key.is_empty() {
            req = req.bearer_auth(key);
        }
    }

    let response = req
        .send()
        .await
        .map_err(|e| format!("请求失败: {}", e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(format!("LLM 返回错误 {}: {}", status, body));
    }

    let resp_json: Value = response
        .json()
        .await
        .map_err(|e| format!("解析响应失败: {}", e))?;

    let content = resp_json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or("响应格式错误：缺少 content 字段")?;

    // 尝试从内容中提取 JSON（处理思考标签、代码块等）
    let json_str = extract_json(content)?;

    let parsed: ChatAdjustResponse = serde_json::from_str(&json_str)
        .map_err(|e| format!("解析 JSON 失败: {}，原始内容: {}", e, content))?;

    Ok(parsed)
}
