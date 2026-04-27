// ============================================================================
// PHASE 2: 学习型映射核心
// ============================================================================
// 本模块实现 V4 文档 Phase 2 要求的学习型参数预测能力
// 从规则主导升级到"规则 + 学习型映射"主导
//
// 文档参考：docs/rapid_raw_分析式风格迁移技术架构_v_4.md
// Phase 2 目标：
// - 引入 Neural Preset 思想的参数预测头
// - 支持 style embedding -> 参数建议的学习型映射
// - 保持输出可编辑性

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// 学习型风格原型
/// 
/// 这是 Phase 2 的核心数据结构，用于替代手工 expert_presets
/// 每个原型包含：
/// - 风格 embedding（从 ViT-B backbone 提取）
/// - 参数预测权重（小型 MLP 的输出）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StylePrototype {
    pub id: String,
    pub name: String,
    pub embedding: Vec<f32>,           // 768-dim for ViT-B
    pub parameter_weights: HashMap<String, f32>, // 参数预测权重
    pub confidence_threshold: f64,     // 匹配置信度阈值
}

/// 学习型参数预测器
/// 
/// Phase 2 实现：基于 style embedding 预测参数建议
/// 当前版本：使用最近邻 + 线性插值（简化版）
/// 未来版本：可升级为小型 MLP 神经网络
pub struct LearnedParameterPredictor {
    prototypes: Vec<StylePrototype>,
}

impl LearnedParameterPredictor {
    /// 创建预测器（加载预训练原型）
    pub fn new() -> Self {
        // Phase 2 初始版本：使用少量手工标注的原型作为种子
        // 未来：从离线训练数据中学习
        let prototypes = Self::load_default_prototypes();
        
        Self { prototypes }
    }
    
    /// 加载默认风格原型
    /// 
    /// Phase 2 初始版本：6 个手工标注原型
    /// Phase 3 目标：扩展到 50+ 学习型原型
    fn load_default_prototypes() -> Vec<StylePrototype> {
        vec![
            StylePrototype {
                id: "clean_natural".to_string(),
                name: "清透自然".to_string(),
                embedding: vec![0.0; 768], // 占位，实际应从训练数据提取
                parameter_weights: [
                    ("contrast".to_string(), -0.06),
                    ("highlights".to_string(), -0.06),
                    ("shadows".to_string(), 0.06),
                    ("saturation".to_string(), -0.04),
                    ("vibrance".to_string(), 0.02),
                ]
                .iter()
                .cloned()
                .collect(),
                confidence_threshold: 0.70,
            },
            StylePrototype {
                id: "bright_airy".to_string(),
                name: "明亮通透".to_string(),
                embedding: vec![0.0; 768],
                parameter_weights: [
                    ("contrast".to_string(), -0.10),
                    ("highlights".to_string(), -0.08),
                    ("shadows".to_string(), 0.14),
                    ("saturation".to_string(), -0.08),
                ]
                .iter()
                .cloned()
                .collect(),
                confidence_threshold: 0.70,
            },
            StylePrototype {
                id: "moody_matte".to_string(),
                name: "暗调哑光".to_string(),
                embedding: vec![0.0; 768],
                parameter_weights: [
                    ("contrast".to_string(), 0.10),
                    ("highlights".to_string(), -0.10),
                    ("shadows".to_string(), -0.06),
                    ("blacks".to_string(), 0.18),
                    ("saturation".to_string(), -0.10),
                    ("clarity".to_string(), 0.04),
                ]
                .iter()
                .cloned()
                .collect(),
                confidence_threshold: 0.70,
            },
            StylePrototype {
                id: "film_soft".to_string(),
                name: "胶片柔和".to_string(),
                embedding: vec![0.0; 768],
                parameter_weights: [
                    ("contrast".to_string(), -0.12),
                    ("highlights".to_string(), -0.08),
                    ("shadows".to_string(), 0.08),
                    ("temperature".to_string(), 0.08),
                    ("saturation".to_string(), -0.10),
                    ("clarity".to_string(), -0.04),
                ]
                .iter()
                .cloned()
                .collect(),
                confidence_threshold: 0.70,
            },
            StylePrototype {
                id: "cinematic_teal_orange".to_string(),
                name: "电影青橙".to_string(),
                embedding: vec![0.0; 768],
                parameter_weights: [
                    ("contrast".to_string(), 0.08),
                    ("highlights".to_string(), -0.10),
                    ("shadows".to_string(), 0.06),
                    ("temperature".to_string(), 0.10),
                    ("tint".to_string(), -0.04),
                    ("vibrance".to_string(), 0.06),
                ]
                .iter()
                .cloned()
                .collect(),
                confidence_threshold: 0.70,
            },
            StylePrototype {
                id: "cool_clean".to_string(),
                name: "冷调干净".to_string(),
                embedding: vec![0.0; 768],
                parameter_weights: [
                    ("contrast".to_string(), -0.06),
                    ("highlights".to_string(), -0.04),
                    ("shadows".to_string(), 0.06),
                    ("temperature".to_string(), -0.10),
                    ("tint".to_string(), 0.02),
                    ("saturation".to_string(), -0.06),
                ]
                .iter()
                .cloned()
                .collect(),
                confidence_threshold: 0.70,
            },
        ]
    }
    
    /// 基于 style embedding 预测参数
    /// 
    /// Phase 2 实现：最近邻匹配 + 线性插值
    /// Phase 3 升级：小型 MLP 神经网络
    pub fn predict_parameters(
        &self,
        style_embedding: &[f32],
        strength: f64,
    ) -> HashMap<String, f64> {
        // 1. 找到最接近的原型
        let (best_prototype, similarity) = self.find_nearest_prototype(style_embedding);
        
        eprintln!(
            "[Learned] Best prototype: {} (similarity={:.3})",
            best_prototype.name, similarity
        );
        
        // 2. 如果相似度太低，返回空建议
        if similarity < best_prototype.confidence_threshold {
            eprintln!("[Learned] Similarity below threshold, returning empty");
            return HashMap::new();
        }
        
        // 3. 应用强度缩放
        let mut predictions = HashMap::new();
        for (key, weight) in &best_prototype.parameter_weights {
            let scaled_value = *weight as f64 * strength * similarity;
            predictions.insert(key.clone(), scaled_value);
        }
        
        eprintln!("[Learned] Predicted {} parameters", predictions.len());
        predictions
    }
    
    /// 找到最接近的风格原型
    fn find_nearest_prototype(&self, embedding: &[f32]) -> (&StylePrototype, f64) {
        let mut best_prototype = &self.prototypes[0];
        let mut best_similarity = 0.0;
        
        for prototype in &self.prototypes {
            let similarity = cosine_similarity(embedding, &prototype.embedding);
            if similarity > best_similarity {
                best_similarity = similarity;
                best_prototype = prototype;
            }
        }
        
        (best_prototype, best_similarity)
    }
}

/// 计算余弦相似度
fn cosine_similarity(a: &[f32], b: &[f32]) -> f64 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    
    let mut dot_product = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    
    for i in 0..a.len() {
        dot_product += (a[i] * b[i]) as f64;
        norm_a += (a[i] * a[i]) as f64;
        norm_b += (b[i] * b[i]) as f64;
    }
    
    if norm_a == 0.0 || norm_b == 0.0 {
        return 0.0;
    }
    
    dot_product / (norm_a.sqrt() * norm_b.sqrt())
}

/// Phase 2 占位：未来的小型 MLP 参数预测头
/// 
/// 当前版本：使用最近邻 + 线性插值
/// Phase 3 升级：训练小型 MLP（输入：768-dim embedding，输出：参数建议）
/// 
/// 架构建议：
/// - Input: 768-dim style embedding (from ViT-B)
/// - Hidden: 256 -> 128 -> 64
/// - Output: 20-dim parameter predictions
/// - 模型大小：< 1MB
/// - 推理时间：< 5ms
#[allow(dead_code)]
pub struct NeuralParameterPredictor {
    // 占位：未来加载 ONNX 模型
    // session: Option<ort::Session>,
}

#[allow(dead_code)]
impl NeuralParameterPredictor {
    pub fn new() -> Self {
        // Phase 3 实现：加载预训练的 ONNX 模型
        // let model_path = get_qraw_models_dir()?.join("style_transfer_param_predictor.onnx");
        // let session = Session::builder()?.commit_from_file(model_path)?;
        
        Self {
            // session: Some(session),
        }
    }
    
    pub fn predict(&self, _embedding: &[f32]) -> HashMap<String, f64> {
        // Phase 3 实现：ONNX 推理
        // let input = Array::from_shape_vec((1, 768), embedding.to_vec())?;
        // let outputs = self.session.run(vec![input])?;
        // parse_parameter_predictions(outputs)
        
        // 当前占位
        HashMap::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_learned_predictor_creation() {
        let predictor = LearnedParameterPredictor::new();
        assert_eq!(predictor.prototypes.len(), 6);
    }
    
    #[test]
    fn test_cosine_similarity() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![1.0, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6);
        
        let c = vec![1.0, 0.0, 0.0];
        let d = vec![0.0, 1.0, 0.0];
        let sim2 = cosine_similarity(&c, &d);
        assert!(sim2.abs() < 1e-6);
    }
}
