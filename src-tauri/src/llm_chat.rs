use crate::gpu_processing::{RenderRequest, process_and_get_dynamic_image};
use crate::image_processing::{downscale_f32_image, get_all_adjustments_from_json};
use crate::style_transfer::{
    DynamicConstraintClampRecord, DynamicConstraintDebugInfo,
    build_dynamic_constraint_window_from_image, clamp_value_with_dynamic_window, extract_features,
};
use crate::{AppState, get_or_init_gpu_context};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::Emitter;

const DEFAULT_TEXT_MODEL: &str = "qwen3.5:9b";

fn resolve_text_model(llm_model: Option<String>) -> String {
    match llm_model {
        Some(model) => {
            let trimmed = model.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
                DEFAULT_TEXT_MODEL.to_string()
            } else {
                trimmed.to_string()
            }
        }
        None => DEFAULT_TEXT_MODEL.to_string(),
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct AdjustmentSuggestion {
    pub key: String,
    #[serde(default)]
    pub value: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complex_value: Option<serde_json::Value>,
    #[serde(default)]
    pub label: String,
    #[serde(default)]
    pub min: f64,
    #[serde(default)]
    pub max: f64,
    #[serde(default)]
    pub reason: String,
}

impl AdjustmentSuggestion {
    pub fn get_f64_value(&self) -> f64 {
        if let Some(n) = self.value.as_f64() {
            n
        } else if let Some(n) = self.value.as_i64() {
            n as f64
        } else if let Some(s) = self.value.as_str() {
            s.parse::<f64>().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    pub fn set_f64_value(&mut self, val: f64) {
        self.value = json!(val);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ChatAdjustResponse {
    pub understanding: String,
    pub adjustments: Vec<AdjustmentSuggestion>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style_debug: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraint_debug: Option<Value>,
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

fn current_numeric_value(map: &Value, key: &str) -> f64 {
    map.get(key).and_then(|v| v.as_f64()).unwrap_or(0.0)
}

fn apply_guardrails(
    adjustments: &mut Vec<AdjustmentSuggestion>,
    current: &Value,
    window: &crate::style_transfer::DynamicConstraintWindow,
) -> Vec<DynamicConstraintClampRecord> {
    let mut clamps = Vec::new();

    let cur_exposure = current_numeric_value(current, "exposure");
    let cur_highlights = current_numeric_value(current, "highlights");
    let cur_whites = current_numeric_value(current, "whites");
    let cur_shadows = current_numeric_value(current, "shadows");
    let cur_blacks = current_numeric_value(current, "blacks");
    let cur_saturation = current_numeric_value(current, "saturation");
    let cur_vibrance = current_numeric_value(current, "vibrance");

    let find_idx = |k: &str| {
        adjustments
            .iter()
            .position(|a| a.key == k && a.complex_value.is_none())
    };

    let idx_exposure = find_idx("exposure");
    let idx_highlights = find_idx("highlights");
    let idx_whites = find_idx("whites");
    let idx_shadows = find_idx("shadows");
    let idx_blacks = find_idx("blacks");
    let idx_saturation = find_idx("saturation");
    let idx_vibrance = find_idx("vibrance");

    let mut exposure_delta = idx_exposure
        .map(|i| adjustments[i].get_f64_value() - cur_exposure)
        .unwrap_or(0.0);
    let mut highlights_delta = idx_highlights
        .map(|i| adjustments[i].get_f64_value() - cur_highlights)
        .unwrap_or(0.0);
    let mut whites_delta = idx_whites
        .map(|i| adjustments[i].get_f64_value() - cur_whites)
        .unwrap_or(0.0);

    let mut shadows_delta = idx_shadows
        .map(|i| adjustments[i].get_f64_value() - cur_shadows)
        .unwrap_or(0.0);
    let mut blacks_delta = idx_blacks
        .map(|i| adjustments[i].get_f64_value() - cur_blacks)
        .unwrap_or(0.0);

    let mut saturation_delta = idx_saturation
        .map(|i| adjustments[i].get_f64_value() - cur_saturation)
        .unwrap_or(0.0);
    let mut vibrance_delta = idx_vibrance
        .map(|i| adjustments[i].get_f64_value() - cur_vibrance)
        .unwrap_or(0.0);

    let mut apply_value = |key: &str, idx: Option<usize>, original: f64, value: f64, note: &str| {
        let Some(i) = idx else { return };
        let (clamped, reason) = clamp_value_with_dynamic_window(key, value, window);
        let adj = &mut adjustments[i];
        adj.set_f64_value(clamped);
        let note_text = reason.unwrap_or_else(|| note.to_string());
        adj.reason = if adj.reason.is_empty() {
            note_text.clone()
        } else {
            format!("{}；{}", adj.reason, note_text)
        };
        clamps.push(DynamicConstraintClampRecord {
            key: key.to_string(),
            label: adj.label.clone(),
            original,
            clamped,
            reason: note.to_string(),
        });
    };

    let highlight_push = exposure_delta.max(0.0)
        + (highlights_delta.max(0.0) / 50.0)
        + (whites_delta.max(0.0) / 50.0);
    let highlight_allow = (0.65 - window.highlight_risk * 0.55).max(0.15).min(0.65);
    if highlight_push > highlight_allow && highlight_push.is_finite() {
        let scale = (highlight_allow / highlight_push).max(0.0).min(1.0);
        if exposure_delta > 0.0 {
            let original = cur_exposure + exposure_delta;
            exposure_delta *= scale;
            apply_value(
                "exposure",
                idx_exposure,
                original,
                cur_exposure + exposure_delta,
                "审美护栏：限制提亮组合",
            );
        }
        if highlights_delta > 0.0 {
            let original = cur_highlights + highlights_delta;
            highlights_delta *= scale;
            apply_value(
                "highlights",
                idx_highlights,
                original,
                cur_highlights + highlights_delta,
                "审美护栏：限制提亮组合",
            );
        }
        if whites_delta > 0.0 {
            let original = cur_whites + whites_delta;
            whites_delta *= scale;
            apply_value(
                "whites",
                idx_whites,
                original,
                cur_whites + whites_delta,
                "审美护栏：限制提亮组合",
            );
        }
    }

    let shadow_push = (-exposure_delta).max(0.0)
        + ((-shadows_delta).max(0.0) / 50.0)
        + ((-blacks_delta).max(0.0) / 50.0);
    let shadow_allow = (0.65 - window.shadow_risk * 0.55).max(0.15).min(0.65);
    if shadow_push > shadow_allow && shadow_push.is_finite() {
        let scale = (shadow_allow / shadow_push).max(0.0).min(1.0);
        if exposure_delta < 0.0 {
            let original = cur_exposure + exposure_delta;
            exposure_delta *= scale;
            apply_value(
                "exposure",
                idx_exposure,
                original,
                cur_exposure + exposure_delta,
                "审美护栏：限制压暗组合",
            );
        }
        if shadows_delta < 0.0 {
            let original = cur_shadows + shadows_delta;
            shadows_delta *= scale;
            apply_value(
                "shadows",
                idx_shadows,
                original,
                cur_shadows + shadows_delta,
                "审美护栏：限制压暗组合",
            );
        }
        if blacks_delta < 0.0 {
            let original = cur_blacks + blacks_delta;
            blacks_delta *= scale;
            apply_value(
                "blacks",
                idx_blacks,
                original,
                cur_blacks + blacks_delta,
                "审美护栏：限制压暗组合",
            );
        }
    }

    let sat_push = (saturation_delta.max(0.0) / 40.0) + (vibrance_delta.max(0.0) / 40.0);
    let sat_allow = (0.7 - window.saturation_risk * 0.6).max(0.15).min(0.7);
    if sat_push > sat_allow && sat_push.is_finite() {
        let scale = (sat_allow / sat_push).max(0.0).min(1.0);
        if saturation_delta > 0.0 {
            let original = cur_saturation + saturation_delta;
            saturation_delta *= scale;
            apply_value(
                "saturation",
                idx_saturation,
                original,
                cur_saturation + saturation_delta,
                "审美护栏：限制饱和组合",
            );
        }
        if vibrance_delta > 0.0 {
            let original = cur_vibrance + vibrance_delta;
            vibrance_delta *= scale;
            apply_value(
                "vibrance",
                idx_vibrance,
                original,
                cur_vibrance + vibrance_delta,
                "审美护栏：限制饱和组合",
            );
        }
    }

    clamps
}

fn build_system_prompt(current_adjustments: &Value, constraint_window: &Value) -> String {
    let adj_str = serde_json::to_string_pretty(current_adjustments).unwrap_or_default();
    let constraint_str = serde_json::to_string_pretty(constraint_window).unwrap_or_default();

    format!(
        r#"你是一位专业摄影师助手，帮助用户通过自然语言调整照片参数，并且必须稳定输出可解析的严格 JSON。

## 你的任务
根据用户的描述，推断出最合适的调整参数，以 JSON 格式返回。

## 核心规则
1. value 是最终绝对值，不是增量。例如用户说"提高曝光"，你应该基于当前值计算出新的绝对值。
2. 必须参考"当前图像参数"，基于当前值做调整。用户可能已经手动拖过滑块。
3. 如果用户说"再亮一点"，在当前 exposure 基础上增加，而不是从 0 开始。
4. 如果用户说"太过了"或"回退一点"，在当前值基础上反向微调。
5. 多轮对话以"当前图像参数"为唯一事实来源：历史对话只用于理解意图，不能覆盖当前参数数值。
6. 用户说"只调整X/其它保持不变"时，只输出 X 对应的 key，不要输出其它参数。
7. 用户说"撤销/回到上一步/恢复一点"时，把相关参数向当前值回退 30%~50%，输出回退后的最终绝对值。
8. 不确定时宁可不调：允许返回空数组 adjustments: []。

## 允许的参数 key（白名单，禁止输出其它 key）
- 全局调整: exposure, brightness, contrast, highlights, shadows, whites, blacks, saturation, vibrance, temperature, tint, clarity, dehaze, structure, sharpness, vignetteAmount
- HSL 颜色调整 (只输出对应的颜色层): hsl

如果用户想要调整特定颜色（例如："将橙色转换为金黄色"或"让天空更蓝"），你必须输出 `hsl` 对象。
- hsl 的 `complex_value` 格式：`{{"<color>": {{"hue": <偏移值>, "saturation": <增量>, "luminance": <增量>}}}}`
- `<color>` 必须从以下 8 种中选择：`reds, oranges, yellows, greens, aquas, blues, purples, magentas`
- 偏移值范围：-100 到 100（整数）。
例如将橙色变黄：`{{"key": "hsl", "complex_value": {{"oranges": {{"hue": 20, "saturation": 0, "luminance": 0}}}}, "label": "HSL颜色", "reason": "将橙色偏移向黄色"}}`

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

## 数值与步长（必须遵守）
- exposure 必须是 0.01 的倍数（例如 0.23、-1.07）
- 其它整数步长参数必须输出整数（不要输出 12.3 这类小数）
- 所有数值必须是合法 JSON number（禁止 NaN / Infinity / 字符串数字）

## 当前图像参数
```json
{adj_str}
```

## 当前图像动态约束窗口（必须遵守）
```json
{constraint_str}
```

## 输出格式（严格 JSON，禁止输出任何其他文字/Markdown/代码块）
- 最终回复必须是单个 JSON 对象，前后不允许任何多余字符
- 禁止使用 ```json 代码块、禁止注释、禁止尾随逗号
- 字段名与字符串必须使用双引号

{{
  "understanding": "用一句话描述你对用户意图的理解",
  "adjustments": [
    {{
      "key": "参数键名 (例如 hsl 或 exposure)",
      "value": 数值 (如果是数字),
      "complex_value": 复杂对象 (仅当 key 为 hsl 时提供，如 {{"oranges": {{"hue": 10, "saturation": 0, "luminance": 0}}}}),
      "label": "中文参数名",
      "min": 最小值,
      "max": 最大值,
      "reason": "调整原因（简短）"
    }}
  ]
}}

只返回需要调整的参数（与当前值不同的），最多返回8个最重要的参数。
如果无需调整，返回：
{{ "understanding": "...", "adjustments": [] }}

## 安全约束（必须遵守）
- exposure 绝对值不得超过 2.5（即 -2.5 ~ 2.5）
- 其他参数绝对值不得超过 80（即 -80 ~ 80）
- 用户说"太亮/太暗/太过了"时，只做小幅调整（exposure ±0.3~0.8，其他参数 ±10~30）
- 绝对禁止把 exposure 设为 3.0 以上或 -3.0 以下
- 注意：如果当前图片为全白（曝光过度），你必须把 exposure、highlights 和 whites 的值设置为极低的负数（如 exposure -2.5, highlights -100），以拉回画面细节。
- 如果用户的描述模糊，宁可保守调整也不要激进
- 输出值必须落在动态约束窗口 bands 对应的 hard_min/hard_max 内 (对于 HSL, min 为 -100, max 为 100)
- 对于普通数值参数，min/max 必须等于该 key 在动态约束窗口 bands 中的 hard_min/hard_max；如果 bands 中不存在该 key，则不要输出该 adjustment"#,
        adj_str = adj_str,
        constraint_str = constraint_str
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
    current_image_path: Option<String>,
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<ChatAdjustResponse, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;

    let model = resolve_text_model(llm_model);
    let constraint_window = build_dynamic_constraint_window_from_image(
        current_image_path.as_deref(),
        &current_adjustments,
    );
    let constraint_window_value = serde_json::to_value(&constraint_window)
        .map_err(|e| format!("动态约束序列化失败: {}", e))?;
    let system_prompt = build_system_prompt(&current_adjustments, &constraint_window_value);

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
                    // qwen3.6:27b 使用 reasoning 字段存储思考过程，content 字段存储最终答案
                    // 我们需要同时处理这两个字段
                    let delta_reasoning = sse_json["choices"][0]["delta"]["reasoning"].as_str();
                    let delta_content = sse_json["choices"][0]["delta"]["content"].as_str();
                    
                    // 优先使用 reasoning（思考过程），如果没有则使用 content
                    let text_to_process = delta_reasoning.or(delta_content);
                    
                    if let Some(delta_text) = text_to_process {
                        if delta_text.is_empty() {
                            continue;
                        }

                        full_content.push_str(delta_text);

                        // 检测 <think> 标签状态
                        if delta_text.contains("<think>") {
                            in_thinking = true;
                        }

                        if in_thinking {
                            thinking_buffer.push_str(delta_text);
                            // 清理标签后推送思考内容
                            let clean =
                                delta_text.replace("<think>", "").replace("</think>", "");
                            if !clean.is_empty() {
                                let _ = app_handle.emit(
                                    "chat-stream-chunk",
                                    StreamChunkPayload {
                                        chunk_type: "thinking".to_string(),
                                        text: clean,
                                        result: None,
                                    },
                                );
                            }
                        }

                        if delta_text.contains("</think>") {
                            in_thinking = false;
                            thinking_buffer.clear();
                            // 思考结束，推送过渡提示
                            let _ = app_handle.emit(
                                "chat-stream-chunk",
                                StreamChunkPayload {
                                    chunk_type: "thinking".to_string(),
                                    text: "\n正在生成调整参数...\n".to_string(),
                                    result: None,
                                },
                            );
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
    let mut parsed: ChatAdjustResponse = serde_json::from_str(&json_str).map_err(|e| {
        format!(
            "解析 JSON 失败: {}，原始内容: {}",
            e,
            &full_content[..full_content.len().min(500)]
        )
    })?;

    let mut clamps = Vec::new();
    for adj in &mut parsed.adjustments {
        // 跳过复杂对象 (如 hsl) 的动态边界检查
        if adj.key == "hsl" {
            continue;
        }

        if let Some(band) = constraint_window.bands.get(&adj.key) {
            adj.min = adj.min.max(band.hard_min);
            adj.max = adj.max.min(band.hard_max);
            if adj.min > adj.max {
                adj.min = band.hard_min;
                adj.max = band.hard_max;
            }
        }
        let original = adj.get_f64_value();
        let (clamped, reason) =
            clamp_value_with_dynamic_window(&adj.key, original, &constraint_window);
        adj.set_f64_value(clamped);
        if let Some(reason_text) = reason {
            clamps.push(DynamicConstraintClampRecord {
                key: adj.key.clone(),
                label: adj.label.clone(),
                original,
                clamped,
                reason: reason_text,
            });
            if adj.reason.is_empty() {
                adj.reason = "动态约束已调整".to_string();
            } else {
                adj.reason = format!("{}；动态约束已调整", adj.reason);
            }
        }
    }

    let guardrail_clamps = apply_guardrails(
        &mut parsed.adjustments,
        &current_adjustments,
        &constraint_window,
    );
    clamps.extend(guardrail_clamps);

    let constraint_debug = DynamicConstraintDebugInfo {
        window: constraint_window.clone(),
        clamp_count: clamps.len(),
        clamps: clamps.clone(),
    };
    parsed.constraint_debug = serde_json::to_value(&constraint_debug).ok();

    if let Some(ref path) = current_image_path {
        if let Ok(context) = get_or_init_gpu_context(&state) {
            if let Ok(loaded) = state.original_image.lock() {
                if let Some(original) = &*loaded {
                    if original.path == *path {
                        let mut merged_adjustments = current_adjustments.clone();
                        if let Some(obj) = merged_adjustments.as_object_mut() {
                            for adj in &parsed.adjustments {
                                if adj.key == "hsl" && adj.complex_value.is_some() {
                                    if let Some(v) = &adj.complex_value {
                                        obj.insert(adj.key.clone(), v.clone());
                                    }
                                } else {
                                    obj.insert(adj.key.clone(), json!(adj.value));
                                }
                            }
                        }
                        let preview_base = downscale_f32_image(&original.image, 200, 200);
                        let all_adjustments =
                            get_all_adjustments_from_json(&merged_adjustments, original.is_raw);
                        if let Ok(processed_image) = process_and_get_dynamic_image(
                            &context,
                            &state,
                            &preview_base,
                            0,
                            RenderRequest {
                                adjustments: all_adjustments,
                                mask_bitmaps: &[],
                                lut: None,
                                roi: None,
                            },
                            "chat_adjust_verification",
                        ) {
                            let result_features = extract_features(&processed_image);
                            let mut needed_fix = false;
                            if result_features.clipped_highlight_ratio > 0.10 {
                                if let Some(adj) =
                                    parsed.adjustments.iter_mut().find(|a| a.key == "exposure")
                                {
                                    adj.set_f64_value(adj.get_f64_value() - 0.5);
                                    adj.reason = format!(
                                        "{}；图像域验证：触发过曝自愈，强制降曝光",
                                        adj.reason
                                    );
                                    needed_fix = true;
                                }
                            }
                            if result_features.shadow_ratio > 0.30
                                && result_features.p10_luminance < 0.05
                            {
                                if let Some(adj) =
                                    parsed.adjustments.iter_mut().find(|a| a.key == "shadows")
                                {
                                    adj.set_f64_value(adj.get_f64_value() + 20.0);
                                    adj.reason = format!(
                                        "{}；图像域验证：触发死黑自愈，强制拉阴影",
                                        adj.reason
                                    );
                                    needed_fix = true;
                                }
                            }
                            if result_features.mean_saturation > 0.65 {
                                if let Some(adj) =
                                    parsed.adjustments.iter_mut().find(|a| a.key == "vibrance")
                                {
                                    adj.set_f64_value(adj.get_f64_value() - 20.0);
                                    adj.reason = format!(
                                        "{}；图像域验证：触发超饱和自愈，强制降饱和",
                                        adj.reason
                                    );
                                    needed_fix = true;
                                }
                            }
                            if needed_fix {
                                if let Some(dbg) = parsed
                                    .constraint_debug
                                    .as_mut()
                                    .and_then(|v| v.as_object_mut())
                                {
                                    dbg.insert("image_verified".to_string(), json!(true));
                                    dbg.insert("auto_corrected".to_string(), json!(true));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let _ = app_handle.emit(
        "chat-stream-chunk",
        StreamChunkPayload {
            chunk_type: "done".to_string(),
            text: String::new(),
            result: Some(parsed.clone()),
        },
    );

    Ok(parsed)
}
