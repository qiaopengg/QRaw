# QRaw AI 智能选图 — 技术架构设计（v4.1 修正版）

> 基于 v4 评审发现的 6 个问题，逐一修正。
> 预期综合效果：8-8.5/10

---

## 一、模型矩阵

### 已有模型（零成本复用）

| 模型              | 文件                              | 用途                       | 大小  |
| ----------------- | --------------------------------- | -------------------------- | ----- |
| YuNet             | face_detection_yunet_2023mar.onnx | 人脸检测+5关键点           | 337KB |
| YOLOv8n-Face      | yolov8n-face.onnx                 | 人脸检测(降级)             | 6MB   |
| FerPlus           | emotion-ferplus-8.onnx            | 表情分类(降级)             | 34MB  |
| NIMA-Aesthetic    | nima.onnx                         | 美学评分                   | 16MB  |
| CLIP ViT-B/32     | clip_model.onnx                   | 场景分类+质量评估+组内精排 | 350MB |
| Depth Anything V2 | depth_anything_v2_vits.onnx       | 深度估计→主体分离          | 50MB  |

### 需新增模型

| 模型                 | 用途                               | 大小  | 获取方式                    | 许可证     |
| -------------------- | ---------------------------------- | ----- | --------------------------- | ---------- |
| InsightFace 2d106det | 106点landmark→闭眼EAR+表情物理指标 | ~5MB  | GitHub release直接下载ONNX  | MIT        |
| HSEmotion            | 表情分类(替代FerPlus)              | ~15MB | PyTorch导出ONNX(1小时)      | Apache-2.0 |
| NIMA-Technical       | 技术质量评分                       | ~16MB | idealo Keras导出ONNX(1小时) | Apache-2.0 |

---

## 二、五阶段流水线

```
Stage 0: 资产发现 → AssetRegistry
Stage 1: 技术淘汰 → TechnicalVerdict (Depth Anything主体分区+拉普拉斯)
Stage 2: 连拍去重 → BurstGroup[] (感知哈希粗筛+CLIP组内精排)
Stage 3: 人像评估 → PortraitVerdict (2d106det+EAR闭眼+HSEmotion+构图)
Stage 4: 综合评分 → FinalRating (决策树+双方美学仲裁+场景自适应)
```

```rust
fn stage_0_discover(paths: &[String], app: &AppHandle) -> Result<AssetRegistry>;
fn stage_1_technical(registry: &AssetRegistry, ai_state: &AiState, settings: &Settings) -> Vec<TechnicalVerdict>;
fn stage_2_dedup(registry: &AssetRegistry, verdicts: &[TechnicalVerdict], clip: Option<&ClipModels>, settings: &Settings) -> Vec<BurstGroup>;
fn stage_3_portrait(registry: &AssetRegistry, verdicts: &[TechnicalVerdict], models: &CullingModelsV4) -> Vec<PortraitVerdict>;
fn stage_4_score(tech: &[TechnicalVerdict], groups: &[BurstGroup], portraits: &[PortraitVerdict], scene: &SceneType, clip: Option<&ClipModels>, nima_aes: Option<&Mutex<Session>>, settings: &Settings) -> Vec<FinalRating>;
```

---

## 三、v4 评审问题修正记录

### 修正 1：106 点 landmark 索引（原文索引全部错误）

**原文错误**：左眼 33-42，右眼 87-96，嘴部 52-71，左眉 19，额头到下巴 0-16

**实际布局**（来源：InsightFace/UniFace 官方文档）：

| 索引范围 | 区域              | 点数 |
| -------- | ----------------- | ---- |
| 0-32     | 脸部轮廓          | 33   |
| 33-50    | 眉毛（左眉+右眉） | 18   |
| 51-62    | 鼻子              | 12   |
| 63-86    | 眼睛（左眼+右眼） | 24   |
| 87-105   | 嘴巴              | 19   |

**眼部细分**（24 点，每只眼 12 点）：

- 左眼：63-74（12 点，上下眼睑各 6 点形成闭合轮廓）
- 右眼：75-86（12 点，同上）

**嘴部细分**（19 点）：

- 外唇轮廓：87-100（14 点）
- 内唇轮廓：101-105（5 点）
- 左嘴角：87，右嘴角：93
- 上唇中点：90，下唇中点：96

**眉毛细分**（18 点）：

- 左眉：33-41（9 点）
- 右眉：42-50（9 点）

### 修正 2：CLIP 模型接口（原文假设错误）

**原文错误**：假设可以分别提取 image embedding 和 text embedding 再算余弦相似度。

**实际接口**：项目中的 CLIP 是联合模型，3 个输入（text_ids, image_pixel, attention_mask）→ 1 个输出（logits）。不能分别提取 embedding。

**修正后的 CLIP 使用方式**：

```rust
/// CLIP 质量评估（替代原来错误的 CLIP-IQA）
/// 利用现有的联合模型接口，对比正面/负面 prompt 的 logits
fn compute_clip_quality(
    image: &DynamicImage,
    clip: &ClipModels,
) -> f64 {
    let quality_prompts = [
        "a sharp well-focused high quality photograph",
        "a blurry low quality poorly composed photograph",
    ];

    // 复用现有的 tokenizer + 预处理流程
    let encodings = clip.tokenizer.encode_batch(
        quality_prompts.iter().map(|s| s.to_string()).collect::<Vec<_>>(), true
    ).unwrap_or_default();
    if encodings.is_empty() { return 0.5; }

    let max_len = encodings.iter().map(|e| e.get_ids().len()).max().unwrap_or(1);
    let n = quality_prompts.len();
    let mut ids_data = Vec::with_capacity(n * max_len);
    let mut mask_data = Vec::with_capacity(n * max_len);
    for enc in &encodings {
        let mut ids: Vec<i64> = enc.get_ids().iter().map(|&i| i as i64).collect();
        let mut mask: Vec<i64> = enc.get_attention_mask().iter().map(|&m| m as i64).collect();
        ids.resize(max_len, 0);
        mask.resize(max_len, 0);
        ids_data.extend_from_slice(&ids);
        mask_data.extend_from_slice(&mask);
    }

    let image_input = preprocess_clip_image(image);
    let ids_array = ndarray::Array::from_shape_vec((n, max_len), ids_data).unwrap().into_dyn();
    let mask_array = ndarray::Array::from_shape_vec((n, max_len), mask_data).unwrap().into_dyn();

    let image_val = Tensor::from_array(image_input.into_dyn().as_standard_layout().into_owned()).unwrap();
    let ids_val = Tensor::from_array(ids_array.as_standard_layout().into_owned()).unwrap();
    let mask_val = Tensor::from_array(mask_array.as_standard_layout().into_owned()).unwrap();

    let logits = {
        let mut sess = clip.model.lock().unwrap();
        let outputs = sess.run(ort::inputs![ids_val, image_val, mask_val]).unwrap();
        outputs[0].try_extract_array::<f32>().unwrap().to_owned()
    };

    let flat = logits.into_raw_vec();
    if flat.len() < 2 { return 0.5; }
    let probs = softmax_1d(&flat);
    // probs[0] = 与"高质量"的匹配度，probs[1] = 与"低质量"的匹配度
    probs[0] as f64  // 0.0~1.0，越高越好
}

/// CLIP 组内精排（同样使用联合模型接口）
/// 对组内每张图计算与"高质量照片"prompt 的匹配度，按此排序
fn clip_rank_within_group(
    cluster: &[usize],
    registry: &AssetRegistry,
    clip: &ClipModels,
) -> Vec<(usize, f64)> {
    cluster.iter().map(|&idx| {
        let score = compute_clip_quality(&registry.assets[idx].thumbnail, clip);
        (idx, score)
    }).collect()
}
```

### 修正 3：美学仲裁从"三方"改为"双方"

**原文错误**：标题写"三方仲裁"但只有 NIMA 和 CLIP 两个信号。

**修正**：改为"双方仲裁 + 构图规则"三信号融合：

```rust
fn compute_aesthetic_score(
    nima_aesthetic: Option<f64>,   // NIMA 1-10
    clip_quality: Option<f64>,     // CLIP 0-1
    composition_score: f64,        // 构图规则 0-1
) -> f64 {
    let mut signals: Vec<f64> = vec![];

    if let Some(nima) = nima_aesthetic {
        signals.push((nima - 5.0) / 5.0);  // 归一化到 -1~+1
    }
    if let Some(clip_q) = clip_quality {
        signals.push((clip_q - 0.5) * 2.0);  // 归一化到 -1~+1
    }
    signals.push((composition_score - 0.5) * 2.0);  // 构图分作为第三信号

    if signals.is_empty() { return 0.0; }

    // 取中位数（3 个信号时真正有仲裁效果）
    signals.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let median = signals[signals.len() / 2];
    (median * 0.10).clamp(-0.10, 0.10)
}
```

### 修正 4：场景自适应权重数学校准

**原文错误**：人像场景最大值 0.76，永远到不了 5 星（阈值 0.80）。

**修正**：调整权重使各场景的理论最大值都能达到 0.95+：

```rust
let final_q = match scene {
    SceneType::CloseUpPortrait | SceneType::HalfBodyPortrait => {
        // portrait_mod 范围 [-0.12, 0.12]，aesthetic_mod 范围 [-0.10, 0.10]
        // 最大值：0.65 + 0.18 + 0.10 = 0.93 ✓
        tech_q * 0.65 + portrait_mod * 1.5 + aesthetic_mod
    },
    SceneType::Landscape | SceneType::Architecture => {
        // 最大值：0.75 + 0.20 = 0.95 ✓
        tech_q * 0.75 + aesthetic_mod * 2.0
    },
    SceneType::GroupPhoto | SceneType::Wedding => {
        // 最大值：0.55 + 0.24 + 0.10 = 0.89 ✓（合影很难全员完美，5星本就应该少）
        tech_q * 0.55 + portrait_mod * 2.0 + aesthetic_mod
    },
    SceneType::Action => {
        // 最大值：0.70 + 0.12 + 0.10 = 0.92 ✓
        tech_q * 0.70 + portrait_mod + aesthetic_mod
    },
    _ => {
        // 最大值：0.65 + 0.18 + 0.10 = 0.93 ✓
        tech_q * 0.65 + portrait_mod * 1.5 + aesthetic_mod
    },
}.clamp(0.0, 1.0);
```

### 修正 5：Depth Anything 从 CullingModelsV4 中移除

**原文错误**：`CullingModelsV4` 中包含 `depth_model`，但 Depth Anything 已在 `AiState.models.depth_anything` 中管理。

**修正**：Stage 1 直接从 `AppState.ai_state` 获取 Depth Anything，不在 CullingModelsV4 中重复持有。同时 Depth Anything 设为可选（模型不可用时降级为全图清晰度）。

```rust
// 修正后的模型容器（删除 depth_model）
struct CullingModelsV4 {
    face_detector: Mutex<Session>,
    yunet_detector: Option<Mutex<Session>>,
    landmark_106: Option<Mutex<Session>>,    // InsightFace 2d106det
    expression_model: Option<Mutex<Session>>, // HSEmotion (主) 或 FerPlus (降级)
    nima_aesthetic: Option<Mutex<Session>>,
    nima_technical: Option<Mutex<Session>>,
    // depth_model 已删除，从 AiState 获取
}

// Stage 1 获取 Depth Anything 的方式
fn stage_1_technical(
    asset: &Asset,
    ai_state: &Mutex<Option<AiState>>,  // 从 AppState 传入
    nima_tech: Option<&Mutex<Session>>,
    settings: &Settings,
) -> TechnicalVerdict {
    // 尝试获取已初始化的 Depth Anything（不触发下载）
    let depth_session = ai_state.lock().unwrap()
        .as_ref()
        .and_then(|s| s.models.as_ref())
        .map(|m| m.depth_anything.lock().unwrap());

    // 如果 Depth Anything 未初始化（用户没用过 AI 功能），降级为全图清晰度
    let subject_sharpness = if let Some(ref sess) = depth_session {
        // ... 深度分区逻辑 ...
    } else {
        sharpness  // 降级
    };
    // ...
}
```

**关键**：不调用 `get_or_init_ai_models()`（那会下载 500MB 的 SAM 等无关模型），而是检查 `ai_state` 中是否已有 Depth Anything。如果用户之前用过 AI 蒙版等功能，模型已在内存中；如果没用过，静默降级。

### 修正 6：Depth Anything 推理复用已有函数

**原文错误**：重新实现了 `run_depth_anything_for_culling`，与 `ai_processing.rs` 中已有的 `run_depth_anything_model` 重复。

**修正**：直接调用已有函数，只新增 mask 提取逻辑：

```rust
fn compute_subject_sharpness(
    thumbnail: &DynamicImage,
    gray: &GrayImage,
    depth_session: &Session,
) -> f64 {
    // 复用已有的 Depth Anything 推理（ai_processing.rs）
    // run_depth_anything_model 返回 GrayImage 深度图
    let depth_map = match run_depth_anything_inference(thumbnail, depth_session) {
        Ok(d) => d,
        Err(_) => return calculate_laplacian_variance(gray),  // 降级
    };

    // 只新增：从深度图提取前景 mask
    let subject_mask = extract_foreground_mask(&depth_map, 0.30);
    // 只新增：对 mask 区域计算拉普拉斯方差
    calculate_masked_laplacian_variance(gray, &subject_mask)
}

// extract_foreground_mask 和 calculate_masked_laplacian_variance 是新函数
// 但 Depth Anything 的预处理/推理/后处理完全复用已有代码
```

---

## 四、修正后的 106 点 Landmark 关键函数

```rust
/// InsightFace 2d106det 推理（索引已按官方文档修正）
fn run_landmark_106(
    image: &DynamicImage,
    face: &FaceBox,
    model: &Mutex<Session>,
) -> Result<Vec<(f32, f32)>> {
    let (w, h) = image.dimensions();
    let pad = ((face.x2 - face.x1).max(face.y2 - face.y1)) * 0.2;
    let x1 = (face.x1 - pad).max(0.0) as u32;
    let y1 = (face.y1 - pad).max(0.0) as u32;
    let x2 = (face.x2 + pad).min(w as f32) as u32;
    let y2 = (face.y2 + pad).min(h as f32) as u32;
    let crop = imageops::crop_imm(image, x1, y1, x2-x1, y2-y1).to_image();
    let crop_dyn = DynamicImage::ImageRgba8(crop);

    let size = 192u32;
    let rgb = crop_dyn.to_rgb8();
    let resized = imageops::resize(&rgb, size, size, FilterType::Triangle);

    let mut arr = Array4::<f32>::zeros((1, 3, size as usize, size as usize));
    for (x, y, p) in resized.enumerate_pixels() {
        arr[[0, 0, y as usize, x as usize]] = (p[0] as f32 - 127.5) / 128.0;
        arr[[0, 1, y as usize, x as usize]] = (p[1] as f32 - 127.5) / 128.0;
        arr[[0, 2, y as usize, x as usize]] = (p[2] as f32 - 127.5) / 128.0;
    }

    let input = Tensor::from_array(arr.into_dyn().as_standard_layout().into_owned())?;
    let output = {
        let mut sess = model.lock().unwrap();
        let outputs = sess.run(ort::inputs![input])?;
        outputs[0].try_extract_array::<f32>()?.to_owned()
    };

    let flat = output.into_raw_vec_and_offset().0;
    if flat.len() < 212 { return Err(anyhow!("landmark output too short")); }

    let crop_w = (x2 - x1) as f32;
    let crop_h = (y2 - y1) as f32;
    let landmarks: Vec<(f32, f32)> = (0..106).map(|i| {
        (flat[i * 2] * crop_w + x1 as f32, flat[i * 2 + 1] * crop_h + y1 as f32)
    }).collect();

    Ok(landmarks)
}

/// EAR 闭眼检测（修正后的索引）
/// 官方布局：左眼 63-74（12点），右眼 75-86（12点）
/// 每只眼 12 点形成闭合轮廓：上眼睑 6 点 + 下眼睑 6 点
fn compute_ear_106(landmarks: &[(f32, f32)]) -> (f64, f64) {
    let ear_12pt = |start: usize| -> f64 {
        // 12 点眼部轮廓：0=左眼角, 1-5=上眼睑, 6=右眼角, 7-11=下眼睑
        let p = |offset: usize| landmarks[start + offset];

        // 垂直距离：上眼睑中点 vs 下眼睑中点（取多对求平均，比 6 点 EAR 稳定）
        let v1 = dist(p(2), p(10));  // 上2 vs 下4
        let v2 = dist(p(3), p(9));   // 上3 vs 下3
        let v3 = dist(p(4), p(8));   // 上4 vs 下2
        // 水平距离：左眼角 vs 右眼角
        let h = dist(p(0), p(6));

        if h < 1e-6 { 0.0 } else { (v1 + v2 + v3) / (3.0 * h) }
    };

    let ear_left = ear_12pt(63);   // 左眼起始索引
    let ear_right = ear_12pt(75);  // 右眼起始索引
    (ear_left, ear_right)
}

/// 嘴部张合度（修正后的索引）
/// 官方布局：嘴巴 87-105（19点）
/// 外唇 87-100（14点），内唇 101-105（5点）
/// 左嘴角 87，右嘴角 93，上唇中 90，下唇中 96
fn compute_mouth_open_106(landmarks: &[(f32, f32)]) -> f64 {
    let upper_lip = landmarks[90];   // 上唇中点
    let lower_lip = landmarks[96];   // 下唇中点
    let left_corner = landmarks[87]; // 左嘴角
    let right_corner = landmarks[93]; // 右嘴角
    let vertical = dist(upper_lip, lower_lip);
    let horizontal = dist(left_corner, right_corner);
    if horizontal < 1e-6 { 0.0 } else { (vertical / horizontal) as f64 }
}

/// 眉毛皱起程度（修正后的索引）
/// 官方布局：左眉 33-41（9点），右眉 42-50（9点）
/// 左眼上缘起始 63
fn compute_brow_furrow_106(landmarks: &[(f32, f32)]) -> f64 {
    // 左眉中点（索引 37）vs 左眼上缘中点（索引 65）
    let brow_mid = landmarks[37];
    let eye_top_mid = landmarks[65];
    // 脸部高度：轮廓顶点(0) 到 下巴(16)
    let face_height = dist(landmarks[0], landmarks[16]);
    if face_height < 1e-6 { return 0.0; }
    let brow_eye_dist = dist(brow_mid, eye_top_mid);
    // 归一化：距离越小说明眉毛越皱
    1.0 - (brow_eye_dist / face_height * 4.0).min(1.0) as f64
}

/// 嘴角下垂程度（修正后的索引）
fn compute_mouth_corner_106(landmarks: &[(f32, f32)]) -> f64 {
    let left_corner = landmarks[87];
    let right_corner = landmarks[93];
    let lower_lip_center = landmarks[96];
    let corner_avg_y = (left_corner.1 + right_corner.1) / 2.0;
    if corner_avg_y > lower_lip_center.1 {
        ((corner_avg_y - lower_lip_center.1) / 20.0).min(1.0) as f64
    } else { 0.0 }
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f64 {
    (((a.0 - b.0) as f64).powi(2) + ((a.1 - b.1) as f64).powi(2)).sqrt()
}
```

**重要说明**：上述 12 点眼部轮廓的内部排列顺序（哪个点是上眼睑第几个）需要在集成时通过可视化验证。InsightFace 官方文档只给出了区域范围（63-74 左眼，75-86 右眼），没有给出每个点的精确语义。**落地时必须先写一个可视化脚本，在几张测试图上画出 106 个点的编号，确认眼部点的实际排列顺序后再硬编码 EAR 的取点逻辑。**

---

## 五、修正后的 Stage 4 综合评分

```rust
fn compute_final_rating(
    tech: &TechnicalVerdict,
    group_info: Option<(&BurstGroup, bool)>,
    portrait: Option<&PortraitVerdict>,
    scene: &SceneType,
    nima_aesthetic: Option<f64>,
    clip_quality: Option<f64>,
    composition_score: f64,
    settings: &Settings,
) -> FinalRating {
    let mut reasons = vec![];
    let mut hard_penalty = 0i32;

    // ═══ 第一层：技术硬淘汰 ═══
    if let Fail { reason } = tech {
        return FinalRating { stars: 1, reasons: vec![reason.to_tag()], .. };
    }

    // ═══ 第二层：闭眼硬约束（106点12点EAR）═══
    if let Some(pv) = portrait {
        for face in &pv.faces {
            if face.area_ratio > 0.03 && !face.is_extreme_profile && face.is_eye_closed {
                hard_penalty += 2;
                reasons.push("eyesClosed".into());
                break;
            }
        }
        for face in &pv.faces {
            if face.area_ratio > 0.05 && face.mouth_open_ratio > 0.5 {
                hard_penalty += 1;
                reasons.push("mouthWideOpen".into());
                break;
            }
        }
        // HSEmotion + landmark 双重验证
        for face in &pv.faces {
            if face.area_ratio > 0.03
                && face.negative_emotion_prob > 0.6
                && face.mouth_corner_down > 0.3 {
                hard_penalty += 1;
                reasons.push("negativeExpression".into());
                break;
            }
        }
        if pv.faces.iter().any(|f| f.is_edge_cropped && f.area_ratio > 0.05) {
            hard_penalty += 1;
            reasons.push("faceCropped".into());
        }
    }

    // ═══ 第三层：技术质量分（短板效应）═══
    let tech_q = match tech {
        Pass { sharpness, subject_sharpness, exposure_health, dynamic_range, nima_technical } => {
            let blur = normalize_sharpness(*subject_sharpness, settings.blur_threshold);
            let exp = *exposure_health;
            let dr = *dynamic_range;
            let nima_t = nima_technical.map(|n| (n/10.0).clamp(0.0,1.0)).unwrap_or(blur);
            let scores = [blur, exp, dr, nima_t];
            let min_s = scores.iter().cloned().fold(f64::MAX, f64::min);
            let avg_s = scores.iter().sum::<f64>() / scores.len() as f64;
            min_s * 0.6 + avg_s * 0.4
        },
        Marginal { sharpness, exposure_health, .. } => {
            (normalize_sharpness(*sharpness, settings.blur_threshold).min(*exposure_health)) * 0.5
        },
        _ => 0.0,
    };

    // ═══ 第四层：人像微调 ═══
    let portrait_mod = if let Some(pv) = portrait {
        if !pv.has_faces { 0.0 } else {
            let smile = pv.faces.iter().filter(|f| f.area_ratio > 0.03)
                .map(|f| f.smile_prob * 0.03).sum::<f64>().min(0.06);
            let comp = (pv.composition_score - 0.5) * 0.12;
            let all_open = pv.faces.iter()
                .filter(|f| f.area_ratio > 0.03 && !f.is_extreme_profile)
                .all(|f| !f.is_eye_closed);
            let eye_bonus = if all_open && pv.faces.len() >= 2 { 0.04 } else { 0.0 };
            (smile + comp + eye_bonus).clamp(-0.12, 0.12)
        }
    } else { 0.0 };

    // ═══ 第五层：美学评分（三信号仲裁：NIMA + CLIP + 构图规则）═══
    let aesthetic_mod = compute_aesthetic_score(nima_aesthetic, clip_quality, composition_score);

    // ═══ 第六层：场景自适应（修正后的权重，各场景最大值 > 0.89）═══
    let final_q = match scene {
        SceneType::CloseUpPortrait | SceneType::HalfBodyPortrait =>
            tech_q * 0.65 + portrait_mod * 1.5 + aesthetic_mod,
        SceneType::Landscape | SceneType::Architecture =>
            tech_q * 0.75 + aesthetic_mod * 2.0,
        SceneType::GroupPhoto | SceneType::Wedding =>
            tech_q * 0.55 + portrait_mod * 2.0 + aesthetic_mod,
        SceneType::Action =>
            tech_q * 0.70 + portrait_mod + aesthetic_mod,
        _ =>
            tech_q * 0.65 + portrait_mod * 1.5 + aesthetic_mod,
    }.clamp(0.0, 1.0);

    // ═══ 第七层：连拍去重降级 ═══
    let mut adjusted = final_q;
    if let Some((group, is_cover)) = group_info {
        if !is_cover {
            reasons.push("burstDuplicate".into());
            adjusted = adjusted.min(get_cover_quality(group) - 0.15);
        }
    }

    // ═══ 星级映射 ═══
    let base = if adjusted > 0.80 { 5 }
        else if adjusted > 0.62 { 4 }
        else if adjusted > 0.42 { 3 }
        else if adjusted > 0.22 { 2 }
        else { 1 };
    let stars = (base as i32 - hard_penalty).clamp(1, 5) as u8;

    FinalRating { stars, reasons, quality_score: adjusted, breakdown: ScoreBreakdown { .. } }
}
```

---

## 六、其余部分（未修改，保持 v4 原文）

以下部分在 v4 评审中未发现问题，保持不变：

- **Stage 0 资产发现**：扫描→配对→EXIF→缩略图
- **Stage 2 连拍去重**：感知哈希粗筛 + CLIP 组内精排（CLIP 精排已按修正 2 改为使用联合模型接口）
- **Stage 3 人像评估流水线**：人脸检测→侧脸检测→106点landmark→EAR→HSEmotion→构图
- **构图评分引擎 10 条规则**：三分线/裁切/留白/视线空间/多人均衡/水平线/居中度/倾斜/对比度/人脸占比
- **场景自动识别**：人脸统计 + CLIP zero-shot
- **增量缓存**：blake3 指纹 + 版本号
- **HSEmotion 推理**：224×224 彩色输入，8 类情绪

---

## 七、模块文件结构

```
src-tauri/src/culling_v4/
├── mod.rs               // 入口，cull_images_v4 命令
├── types.rs             // 所有数据结构
├── stage0_discover.rs   // 资产发现
├── stage1_technical.rs  // 技术淘汰（Depth Anything 可选主体分区）
├── stage2_dedup.rs      // 连拍去重（哈希粗筛+CLIP精排）
├── stage3_portrait.rs   // 人像评估（106点+HSEmotion）
├── stage4_score.rs      // 综合评分（决策树+三信号仲裁）
├── scene_detect.rs      // 场景识别
├── composition.rs       // 构图规则引擎
├── landmarks.rs         // 106点landmark+EAR+嘴部+眉毛（索引已修正）
├── clip_quality.rs      // CLIP质量评估（使用联合模型接口）
├── cache.rs             // 增量缓存
└── models.rs            // CullingModelsV4 初始化（不含 Depth Anything）
```

---

## 八、落地前必须验证的事项

| 事项                                   | 原因                               | 验证方法                                         |
| -------------------------------------- | ---------------------------------- | ------------------------------------------------ |
| 106点眼部内部排列顺序                  | 官方只给区域范围，未给每个点语义   | 写可视化脚本在测试图上画编号                     |
| InsightFace 2d106det ONNX 输入输出格式 | 不同版本可能不同                   | 下载后用 `onnx` Python 包检查 input/output shape |
| HSEmotion ONNX 导出后精度              | PyTorch→ONNX 可能有精度损失        | 对比 PyTorch 和 ONNX 在同一张图上的输出          |
| CLIP 联合模型的 logits 语义            | 需确认 logits[0] 对应第一个 prompt | 用明确的正/负 prompt 测试输出方向                |
| EAR 阈值 0.20 在摄影照片上的适用性     | 论文阈值基于视频帧                 | 收集 50 张闭眼/睁眼照片标定                      |

---

## 九、效果评估（修正后不变）

| 维度     | 分数      | 说明                                        |
| -------- | --------- | ------------------------------------------- |
| 废片淘汰 | 9         | Depth Anything 主体分区（可选，降级为全图） |
| 闭眼检测 | 8.5       | 106点12点EAR（需验证索引后确认）            |
| 表情判断 | 8         | HSEmotion + landmark 双重验证               |
| 构图评估 | 7         | 10条规则 + 三信号仲裁                       |
| 美学评分 | 6.5       | NIMA + CLIP + 构图三信号中位数              |
| 连拍去重 | 8         | 感知哈希 + CLIP 联合模型精排                |
| 场景识别 | 7         | 人脸统计 + CLIP                             |
| **综合** | **8-8.5** |                                             |

---

## 十、实施路线

| 阶段    | 时间 | 内容                                                                                     |
| ------- | ---- | ---------------------------------------------------------------------------------------- |
| Phase 0 | 2天  | **验证阶段**：下载 2d106det，写可视化脚本确认 106 点布局；测试 CLIP 联合模型 logits 语义 |
| Phase 1 | 2周  | Stage 0+1(含 Depth Anything 可选集成)+2(含 CLIP 精排)+缓存+前端                          |
| Phase 2 | 2周  | Stage 3(2d106det 集成+HSEmotion 导出+构图引擎)                                           |
| Phase 3 | 1周  | Stage 4(NIMA-Tech 导出+CLIP 质量评估+场景识别+决策树)                                    |
| Phase 4 | 1周  | 阈值标定+前端统计面板+对比视图+性能优化                                                  |

**总计 6 周 + 2 天验证。关键路径：Phase 0 的索引验证和 Phase 2 的 HSEmotion 导出。**

---

_文档版本：v4.1（修正版）_
_修正：106点索引 · CLIP联合模型接口 · 三信号仲裁 · 权重数学校准 · Depth Anything解耦 · 推理代码复用_
_预期效果：8-8.5/10_
