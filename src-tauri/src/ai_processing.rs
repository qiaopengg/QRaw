use std::env;
use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use anyhow::Result;
use image::imageops::{self, FilterType};
use image::{
    DynamicImage, GenericImageView, GrayImage, ImageBuffer, Luma, Rgb, Rgb32FImage, Rgba, RgbaImage,
};
use ndarray::{Array, Array4, IxDyn};
use ort::session::Session;
use ort::value::Tensor;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tauri::Emitter;
use tauri::Manager;
use tokenizers::Tokenizer;
use tokio::sync::Mutex as TokioMutex;

const ENCODER_URL: &str = "https://huggingface.co/CyberTimon/RapidRAW-Models/resolve/main/sam_vit_b_01ec64_encoder.onnx?download=true";
const DECODER_URL: &str = "https://huggingface.co/CyberTimon/RapidRAW-Models/resolve/main/sam_vit_b_01ec64_decoder.onnx?download=true";
const ENCODER_FILENAME: &str = "sam_vit_b_01ec64_encoder.onnx";
const DECODER_FILENAME: &str = "sam_vit_b_01ec64_decoder.onnx";
const SAM_INPUT_SIZE: u32 = 1024;
const ENCODER_SHA256: &str = "16ab73d9c824886f0de2938c19df22fb9ec3deebfd0de58e65177e479213d7d1";
const DECODER_SHA256: &str = "85d0d672cf5b7fe763edcde429e5533e62f674af4b15c7d688b7673b0ef00bf7";

const U2NETP_URL: &str =
    "https://huggingface.co/CyberTimon/RapidRAW-Models/resolve/main/u2net.onnx?download=true";
const U2NETP_FILENAME: &str = "u2net.onnx";
const U2NETP_INPUT_SIZE: u32 = 320;
const U2NETP_SHA256: &str = "8d10d2f3bb75ae3b6d527c77944fc5e7dcd94b29809d47a739a7a728a912b491";

const SKYSEG_URL: &str = "https://huggingface.co/CyberTimon/RapidRAW-Models/resolve/main/skyseg-u2net.onnx?download=true";
const SKYSEG_FILENAME: &str = "skyseg_u2net.onnx";
const SKYSEG_INPUT_SIZE: u32 = 320;
const SKYSEG_SHA256: &str = "ab9c34c64c3d821220a2886a4a06da4642ffa14d5b30e8d5339056a089aa1d39";

const CLIP_MODEL_URL: &str =
    "https://huggingface.co/CyberTimon/RapidRAW-Models/resolve/main/clip_model.onnx?download=true";
const CLIP_MODEL_FILENAME: &str = "clip_model.onnx";
const CLIP_TOKENIZER_URL: &str = "https://huggingface.co/CyberTimon/RapidRAW-Models/resolve/main/clip_tokenizer.json?download=true";
const CLIP_TOKENIZER_FILENAME: &str = "clip_tokenizer.json";
const CLIP_MODEL_SHA256: &str = "57879bb1c23cdeb350d23569dd251ed4b740a96d747c529e94a2bb8040ac5d00";

const STYLE_TRANSFER_BACKBONE_URL: &str = "https://huggingface.co/onnx-community/dinov2-base-ONNX/resolve/main/onnx/model.onnx?download=true";
const STYLE_TRANSFER_BACKBONE_FILENAME: &str = "style_transfer_dinov2_vitb.onnx";
const STYLE_TRANSFER_BACKBONE_SHA256: &str =
    "f16115e628d65b7cc7b1e16c504e2af682169aabf3fff4edfe906118f522e204";
const STYLE_TRANSFER_PREPROCESS_URL: &str = "https://huggingface.co/onnx-community/dinov2-base-ONNX/resolve/main/preprocessor_config.json?download=true";
const STYLE_TRANSFER_PREPROCESS_FILENAME: &str = "style_transfer_dinov2_vitb.preprocess.json";
const STYLE_TRANSFER_PREPROCESS_SHA256: &str =
    "14e780d86fa1861f8751f868d7f45425b5feb55c38ca26f152ca5097ab30f828";

const DENOISE_URL: &str = "https://huggingface.co/CyberTimon/RapidRAW-Models/resolve/main/nind_denoise_utnet_684.onnx?download=true";
const DENOISE_FILENAME: &str = "nind_denoise_utnet_684.onnx";
const DENOISE_SHA256: &str = "ee3586279d514df557ff3f7dec6df37fafc51ba5d3a3435b2cc9ac2d9017e7fe";

const LAMA_URL: &str =
    "https://huggingface.co/CyberTimon/RapidRAW-Models/resolve/main/lama_fp16.onnx?download=true";
const LAMA_FILENAME: &str = "lama_fp16.onnx";
const LAMA_SHA256: &str = "2d6be6277c400d6f1b91819737f7c3da935e5c63d1b521d393be1196a2bfa82c";

const DEPTH_URL: &str = "https://huggingface.co/CyberTimon/RapidRAW-Models/resolve/main/depth_anything_v2_vits.onnx?download=true";
const DEPTH_FILENAME: &str = "depth_anything_v2_vits.onnx";
const DEPTH_INPUT_SIZE: u32 = 518;
const DEPTH_SHA256: &str = "d2b11a11c1d4a12b47608fa65a17ee9a4c605b55ee1730c8e3b526304f2562be";

const CULL_FACE_DETECTOR_FILENAME: &str = "yolov8n-face.onnx";
const CULL_EXPRESSION_FILENAME: &str = "emotion-ferplus-8.onnx";
const CULL_AESTHETIC_FILENAME: &str = "nima.onnx";
const CULL_YUNET_FILENAME: &str = "face_detection_yunet_2023mar.onnx";

const DEFAULT_CULL_EXPRESSION_URL: &str = "https://github.com/onnx/models/raw/main/validated/vision/body_analysis/emotion_ferplus/model/emotion-ferplus-8.onnx";
const DEFAULT_CULL_EXPRESSION_SHA256: &str =
    "a2a2ba6a335a3b29c21acb6272f962bd3d47f84952aaffa03b60986e04efa61c";

const DEFAULT_CULL_YUNET_URL: &str = "https://github.com/opencv/opencv_zoo/raw/main/models/face_detection_yunet/face_detection_yunet_2023mar.onnx";
const DEFAULT_CULL_YUNET_SHA256: &str =
    "8f2383e4dd3cfbb4553ea8718107fc0423210dc964f9f4280604804ed2552fa4";

pub struct AiModels {
    pub sam_encoder: Mutex<Session>,
    pub sam_decoder: Mutex<Session>,
    pub u2netp: Mutex<Session>,
    pub sky_seg: Mutex<Session>,
    pub depth_anything: Mutex<Session>,
}

pub struct ClipModels {
    pub model: Mutex<Session>,
    pub tokenizer: Tokenizer,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StyleTransferBackboneSizeConfig {
    #[serde(alias = "shortestEdge")]
    pub shortest_edge: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StyleTransferBackboneCropConfig {
    pub height: u32,
    pub width: u32,
}

#[derive(Clone, Debug, Deserialize)]
pub struct StyleTransferBackbonePreprocessConfig {
    #[serde(alias = "cropSize")]
    pub crop_size: StyleTransferBackboneCropConfig,
    #[serde(alias = "doCenterCrop")]
    pub do_center_crop: bool,
    #[serde(default, alias = "doConvertRgb")]
    pub do_convert_rgb: bool,
    #[serde(alias = "doNormalize")]
    pub do_normalize: bool,
    #[serde(alias = "doRescale")]
    pub do_rescale: bool,
    #[serde(alias = "doResize")]
    pub do_resize: bool,
    #[serde(alias = "imageMean")]
    pub image_mean: Vec<f32>,
    #[serde(alias = "imageStd")]
    pub image_std: Vec<f32>,
    #[serde(alias = "rescaleFactor")]
    pub rescale_factor: f32,
    pub size: StyleTransferBackboneSizeConfig,
}

pub struct StyleTransferBackboneModels {
    pub session: Mutex<Session>,
    pub preprocess: StyleTransferBackbonePreprocessConfig,
}

static STYLE_TRANSFER_BACKBONE_MODELS: OnceLock<Arc<StyleTransferBackboneModels>> = OnceLock::new();

pub struct CullingModels {
    pub face_detector: Mutex<Session>,
    pub yunet_detector: Option<Mutex<Session>>,
    pub expression_model: Option<Mutex<Session>>,
    pub aesthetic_model: Option<Mutex<Session>>,
}

#[derive(Clone)]
pub struct ImageEmbeddings {
    pub path_hash: String,
    pub embeddings: Array<f32, IxDyn>,
    pub original_size: (u32, u32),
}

#[derive(Clone)]
pub struct CachedDepthMap {
    pub path_hash: String,
    pub depth_image: GrayImage,
    pub original_size: (u32, u32),
}

pub struct AiState {
    pub models: Option<Arc<AiModels>>,
    pub denoise_model: Option<Arc<Mutex<Session>>>,
    pub clip_models: Option<Arc<ClipModels>>,
    pub lama_model: Option<Arc<Mutex<Session>>>,
    pub culling_models: Option<Arc<CullingModels>>,
    pub embeddings: Option<ImageEmbeddings>,
    pub depth_map: Option<CachedDepthMap>,
}

fn edt_1d(f: &mut [f32], v: &mut [usize], z: &mut [f32], d: &mut [f32]) {
    let n = f.len();
    if n == 0 {
        return;
    }
    let mut k = 0;
    v[0] = 0;
    z[0] = f32::NEG_INFINITY;
    z[1] = f32::INFINITY;
    for q in 1..n {
        let mut s = ((f[q] + (q * q) as f32) - (f[v[k]] + (v[k] * v[k]) as f32))
            / (2.0 * (q as f32 - v[k] as f32));
        while s <= z[k] {
            if k == 0 {
                break;
            }
            k -= 1;
            s = ((f[q] + (q * q) as f32) - (f[v[k]] + (v[k] * v[k]) as f32))
                / (2.0 * (q as f32 - v[k] as f32));
        }
        k += 1;
        v[k] = q;
        z[k] = s;
        z[k + 1] = f32::INFINITY;
    }
    k = 0;
    for (q, d_q) in d[..n].iter_mut().enumerate() {
        while z[k + 1] < q as f32 {
            k += 1;
        }
        let diff = q as f32 - v[k] as f32;
        *d_q = diff * diff + f[v[k]];
    }
    f.copy_from_slice(&d[..n]);
}

fn edt_2d(grid: &[bool], width: usize, height: usize) -> Vec<f32> {
    let area = width * height;
    let mut f = vec![0.0; area];
    for i in 0..area {
        f[i] = if grid[i] { 1e10 } else { 0.0 };
    }

    let max_dim = width.max(height);
    let mut v = vec![0; max_dim];
    let mut z = vec![0.0; max_dim + 1];
    let mut d = vec![0.0; max_dim];

    for y in 0..height {
        let start = y * width;
        let end = start + width;
        edt_1d(&mut f[start..end], &mut v, &mut z, &mut d);
    }

    let mut col = vec![0.0; height];
    for x in 0..width {
        for y in 0..height {
            col[y] = f[y * width + x];
        }
        edt_1d(&mut col, &mut v, &mut z, &mut d);
        for y in 0..height {
            f[y * width + x] = col[y];
        }
    }

    f.into_iter().map(|v| v.sqrt()).collect()
}

fn get_models_dir(app_handle: &tauri::AppHandle) -> Result<PathBuf> {
    if let Ok(shared_dir) = get_qraw_models_dir() {
        return Ok(shared_dir);
    }
    let models_dir = app_handle.path().app_data_dir()?.join("models");
    if !models_dir.exists() {
        fs::create_dir_all(&models_dir)?;
    }
    Ok(models_dir)
}

pub fn get_qraw_models_dir() -> Result<PathBuf> {
    if let Ok(dir) = env::var("QRAW_MODELS_DIR") {
        let path = PathBuf::from(dir);
        fs::create_dir_all(&path)?;
        return Ok(path);
    }
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .ok_or_else(|| anyhow::anyhow!("Failed to resolve home directory"))?;
    let path = PathBuf::from(home).join(".qraw").join("models");
    fs::create_dir_all(&path)?;
    Ok(path)
}

fn resize_shortest_edge_rgb(
    image: &DynamicImage,
    shortest_edge: u32,
    filter: FilterType,
) -> image::RgbImage {
    let rgb = image.to_rgb8();
    let (w, h) = rgb.dimensions();
    let min_edge = w.min(h).max(1);
    let scale = shortest_edge as f32 / min_edge as f32;
    let target_w = ((w as f32) * scale).round().max(1.0) as u32;
    let target_h = ((h as f32) * scale).round().max(1.0) as u32;
    imageops::resize(&rgb, target_w, target_h, filter)
}

fn center_crop_rgb(image: &image::RgbImage, target_w: u32, target_h: u32) -> image::RgbImage {
    let crop_w = target_w.min(image.width()).max(1);
    let crop_h = target_h.min(image.height()).max(1);
    let offset_x = image.width().saturating_sub(crop_w) / 2;
    let offset_y = image.height().saturating_sub(crop_h) / 2;
    imageops::crop_imm(image, offset_x, offset_y, crop_w, crop_h).to_image()
}

fn preprocess_style_transfer_backbone_image(
    image: &DynamicImage,
    config: &StyleTransferBackbonePreprocessConfig,
) -> Array4<f32> {
    let source_rgb = if config.do_convert_rgb {
        image.to_rgb8()
    } else {
        image.to_rgb8()
    };
    let resized = if config.do_resize {
        let dynamic_rgb = DynamicImage::ImageRgb8(source_rgb);
        resize_shortest_edge_rgb(
            &dynamic_rgb,
            config.size.shortest_edge.max(1),
            FilterType::CatmullRom,
        )
    } else {
        source_rgb
    };
    let cropped = if config.do_center_crop {
        center_crop_rgb(
            &resized,
            config.crop_size.width.max(1),
            config.crop_size.height.max(1),
        )
    } else {
        resized
    };

    let target_w = config.crop_size.width.max(1) as usize;
    let target_h = config.crop_size.height.max(1) as usize;
    let mut arr = Array4::<f32>::zeros((1, 3, target_h, target_w));

    let mean = if config.image_mean.len() >= 3 {
        [
            config.image_mean[0],
            config.image_mean[1],
            config.image_mean[2],
        ]
    } else {
        [0.485, 0.456, 0.406]
    };
    let std = if config.image_std.len() >= 3 {
        [
            config.image_std[0].max(1e-6),
            config.image_std[1].max(1e-6),
            config.image_std[2].max(1e-6),
        ]
    } else {
        [0.229, 0.224, 0.225]
    };
    let rescale = if config.do_rescale {
        config.rescale_factor
    } else {
        1.0
    };

    for (x, y, pixel) in cropped.enumerate_pixels() {
        let xf = x as usize;
        let yf = y as usize;
        let mut channels = [pixel[0] as f32, pixel[1] as f32, pixel[2] as f32];
        for c in 0..3 {
            channels[c] *= rescale;
            if config.do_normalize {
                channels[c] = (channels[c] - mean[c]) / std[c];
            }
            arr[[0, c, yf, xf]] = channels[c];
        }
    }

    arr
}

async fn download_model(url: &str, dest: &Path) -> Result<()> {
    let response = match reqwest::get(url).await {
        Ok(resp) => resp,
        Err(e) => {
            let mirror_enabled = env::var("QRAW_USE_HF_MIRROR")
                .ok()
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(true);
            if mirror_enabled {
                if let Some(rest) = url.strip_prefix("https://huggingface.co/") {
                    let mirror_url = format!("https://hf-mirror.com/{}", rest);
                    reqwest::get(mirror_url).await?
                } else {
                    return Err(e.into());
                }
            } else {
                return Err(e.into());
            }
        }
    };
    let mut file = fs::File::create(dest)?;
    let mut content = Cursor::new(response.bytes().await?);
    std::io::copy(&mut content, &mut file)?;
    Ok(())
}

fn verify_sha256(path: &Path, expected_hash: &str) -> Result<bool> {
    if !path.exists() {
        return Ok(false);
    }
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    io::copy(&mut file, &mut hasher)?;
    let hash = hasher.finalize();
    let hex_hash = format!("{:x}", hash);
    Ok(hex_hash == expected_hash)
}

async fn download_and_verify_model(
    app_handle: &tauri::AppHandle,
    models_dir: &Path,
    filename: &str,
    url: &str,
    expected_hash: &str,
    model_name: &str,
) -> Result<()> {
    let dest_path = models_dir.join(filename);
    let is_valid = verify_sha256(&dest_path, expected_hash)?;

    if !is_valid {
        if dest_path.exists() {
            println!("Model {} has incorrect hash. Re-downloading.", model_name);
            fs::remove_file(&dest_path)?;
        }
        let _ = app_handle.emit("ai-model-download-start", model_name);
        download_model(url, &dest_path).await?;
        let _ = app_handle.emit("ai-model-download-finish", model_name);

        if !verify_sha256(&dest_path, expected_hash)? {
            return Err(anyhow::anyhow!(
                "Failed to verify model {} after download. Hash mismatch.",
                model_name
            ));
        }
    }
    Ok(())
}

pub fn get_model_env(name: &str, key: &str) -> Option<String> {
    let env_key = format!("QRAW_MODEL_{}_{}", name, key);
    env::var(env_key).ok().filter(|v| !v.trim().is_empty())
}

pub async fn ensure_model(
    app_handle: &tauri::AppHandle,
    models_dir: &Path,
    filename: &str,
    model_name: &str,
    url: Option<&str>,
    sha256: Option<&str>,
) -> Result<PathBuf> {
    let path = models_dir.join(filename);
    if path.exists() {
        if let Some(hash) = sha256 {
            if !verify_sha256(&path, hash)? {
                if let Some(u) = url {
                    fs::remove_file(&path).ok();
                    let _ = app_handle.emit("ai-model-download-start", model_name);
                    download_model(u, &path).await?;
                    let _ = app_handle.emit("ai-model-download-finish", model_name);
                    if !verify_sha256(&path, hash)? {
                        return Err(anyhow::anyhow!(
                            "Failed to verify model {} after download.",
                            model_name
                        ));
                    }
                } else {
                    return Err(anyhow::anyhow!(
                        "Model {} hash mismatch and no URL provided.",
                        model_name
                    ));
                }
            }
        }
        return Ok(path);
    }

    if let Some(u) = url {
        let _ = app_handle.emit("ai-model-download-start", model_name);
        download_model(u, &path).await?;
        let _ = app_handle.emit("ai-model-download-finish", model_name);
        if let Some(hash) = sha256 {
            if !verify_sha256(&path, hash)? {
                return Err(anyhow::anyhow!(
                    "Failed to verify model {} after download.",
                    model_name
                ));
            }
        }
        return Ok(path);
    }

    Err(anyhow::anyhow!(
        "Model {} is missing. Put it in {:?} or set QRAW_MODEL_{}_URL.",
        model_name,
        models_dir,
        model_name
    ))
}

pub async fn get_or_init_ai_models(
    app_handle: &tauri::AppHandle,
    ai_state_mutex: &Mutex<Option<AiState>>,
    ai_init_lock: &TokioMutex<()>,
) -> Result<Arc<AiModels>> {
    if let Some(models) = ai_state_mutex
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|state| state.models.clone())
    {
        return Ok(models);
    }

    let _guard = ai_init_lock.lock().await;

    if let Some(models) = ai_state_mutex
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|state| state.models.clone())
    {
        return Ok(models);
    }

    let models_dir = get_models_dir(app_handle)?;

    download_and_verify_model(
        app_handle,
        &models_dir,
        ENCODER_FILENAME,
        ENCODER_URL,
        ENCODER_SHA256,
        "SAM Encoder",
    )
    .await?;
    download_and_verify_model(
        app_handle,
        &models_dir,
        DECODER_FILENAME,
        DECODER_URL,
        DECODER_SHA256,
        "SAM Decoder",
    )
    .await?;
    download_and_verify_model(
        app_handle,
        &models_dir,
        U2NETP_FILENAME,
        U2NETP_URL,
        U2NETP_SHA256,
        "Foreground Model",
    )
    .await?;
    download_and_verify_model(
        app_handle,
        &models_dir,
        SKYSEG_FILENAME,
        SKYSEG_URL,
        SKYSEG_SHA256,
        "Sky Model",
    )
    .await?;
    download_and_verify_model(
        app_handle,
        &models_dir,
        DEPTH_FILENAME,
        DEPTH_URL,
        DEPTH_SHA256,
        "Depth Model",
    )
    .await?;

    let _ = ort::init().with_name("AI").commit();

    let encoder_path = models_dir.join(ENCODER_FILENAME);
    let decoder_path = models_dir.join(DECODER_FILENAME);
    let u2netp_path = models_dir.join(U2NETP_FILENAME);
    let sky_seg_path = models_dir.join(SKYSEG_FILENAME);
    let depth_path = models_dir.join(DEPTH_FILENAME);

    let sam_encoder = Session::builder()?.commit_from_file(encoder_path)?;
    let sam_decoder = Session::builder()?.commit_from_file(decoder_path)?;
    let u2netp = Session::builder()?.commit_from_file(u2netp_path)?;
    let sky_seg = Session::builder()?.commit_from_file(sky_seg_path)?;
    let depth_anything = Session::builder()?.commit_from_file(depth_path)?;

    crate::register_exit_handler();

    let models = Arc::new(AiModels {
        sam_encoder: Mutex::new(sam_encoder),
        sam_decoder: Mutex::new(sam_decoder),
        u2netp: Mutex::new(u2netp),
        sky_seg: Mutex::new(sky_seg),
        depth_anything: Mutex::new(depth_anything),
    });

    let mut ai_state_lock = ai_state_mutex.lock().unwrap();
    if let Some(state) = ai_state_lock.as_mut() {
        state.models = Some(models.clone());
    } else {
        *ai_state_lock = Some(AiState {
            models: Some(models.clone()),
            denoise_model: None,
            clip_models: None,
            lama_model: None,
            culling_models: None,
            embeddings: None,
            depth_map: None,
        });
    }

    Ok(models)
}

pub async fn get_or_init_denoise_model(
    app_handle: &tauri::AppHandle,
    ai_state_mutex: &Mutex<Option<AiState>>,
    ai_init_lock: &TokioMutex<()>,
) -> Result<Arc<Mutex<Session>>> {
    if let Some(denoise_model) = ai_state_mutex
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|state| state.denoise_model.clone())
    {
        return Ok(denoise_model);
    }

    let _guard = ai_init_lock.lock().await;

    if let Some(denoise_model) = ai_state_mutex
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|state| state.denoise_model.clone())
    {
        return Ok(denoise_model);
    }

    let models_dir = get_models_dir(app_handle)?;
    download_and_verify_model(
        app_handle,
        &models_dir,
        DENOISE_FILENAME,
        DENOISE_URL,
        DENOISE_SHA256,
        "NIND Denoise Model",
    )
    .await?;

    let _ = ort::init().with_name("AI-Denoise").commit();
    let model_path = models_dir.join(DENOISE_FILENAME);
    let session = Session::builder()?.commit_from_file(model_path)?;
    let denoise_model = Arc::new(Mutex::new(session));

    crate::register_exit_handler();

    let mut ai_state_lock = ai_state_mutex.lock().unwrap();
    if let Some(state) = ai_state_lock.as_mut() {
        state.denoise_model = Some(denoise_model.clone());
    } else {
        *ai_state_lock = Some(AiState {
            models: None,
            denoise_model: Some(denoise_model.clone()),
            clip_models: None,
            lama_model: None,
            culling_models: None,
            embeddings: None,
            depth_map: None,
        });
    }

    Ok(denoise_model)
}

pub async fn get_or_init_clip_models(
    app_handle: &tauri::AppHandle,
    ai_state_mutex: &Mutex<Option<AiState>>,
    ai_init_lock: &TokioMutex<()>,
) -> Result<Arc<ClipModels>> {
    if let Some(clip_models) = ai_state_mutex
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|state| state.clip_models.clone())
    {
        return Ok(clip_models);
    }

    let _guard = ai_init_lock.lock().await;

    if let Some(clip_models) = ai_state_mutex
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|state| state.clip_models.clone())
    {
        return Ok(clip_models);
    }

    let models_dir = get_models_dir(app_handle)?;

    download_and_verify_model(
        app_handle,
        &models_dir,
        CLIP_MODEL_FILENAME,
        CLIP_MODEL_URL,
        CLIP_MODEL_SHA256,
        "CLIP Model",
    )
    .await?;

    let clip_tokenizer_path = models_dir.join(CLIP_TOKENIZER_FILENAME);
    if !clip_tokenizer_path.exists() {
        let _ = app_handle.emit("ai-model-download-start", "CLIP Tokenizer");
        download_model(CLIP_TOKENIZER_URL, &clip_tokenizer_path).await?;
        let _ = app_handle.emit("ai-model-download-finish", "CLIP Tokenizer");
    }

    let _ = ort::init().with_name("AI-Tagging").commit();
    let clip_model_path = models_dir.join(CLIP_MODEL_FILENAME);
    let model = Mutex::new(Session::builder()?.commit_from_file(clip_model_path)?);
    let tokenizer =
        Tokenizer::from_file(clip_tokenizer_path).map_err(|e| anyhow::anyhow!(e.to_string()))?;

    crate::register_exit_handler();

    let clip_models = Arc::new(ClipModels { model, tokenizer });

    let mut ai_state_lock = ai_state_mutex.lock().unwrap();
    if let Some(state) = ai_state_lock.as_mut() {
        state.clip_models = Some(clip_models.clone());
    } else {
        *ai_state_lock = Some(AiState {
            models: None,
            denoise_model: None,
            clip_models: Some(clip_models.clone()),
            lama_model: None,
            culling_models: None,
            embeddings: None,
            depth_map: None,
        });
    }

    Ok(clip_models)
}

pub async fn get_or_init_style_transfer_backbone(
    app_handle: &tauri::AppHandle,
    ai_init_lock: &TokioMutex<()>,
) -> Result<Arc<StyleTransferBackboneModels>> {
    if let Some(models) = STYLE_TRANSFER_BACKBONE_MODELS.get() {
        return Ok(models.clone());
    }

    let _guard = ai_init_lock.lock().await;

    if let Some(models) = STYLE_TRANSFER_BACKBONE_MODELS.get() {
        return Ok(models.clone());
    }

    let models_dir = get_models_dir(app_handle)?;
    let backbone_url = get_model_env("STYLE_TRANSFER_DINOV2_VITB", "URL")
        .or_else(|| Some(STYLE_TRANSFER_BACKBONE_URL.to_string()));
    let preprocess_url = get_model_env("STYLE_TRANSFER_DINOV2_VITB_PREPROCESS", "URL")
        .or_else(|| Some(STYLE_TRANSFER_PREPROCESS_URL.to_string()));
    let preprocess_sha = get_model_env("STYLE_TRANSFER_DINOV2_VITB_PREPROCESS", "SHA256")
        .or_else(|| Some(STYLE_TRANSFER_PREPROCESS_SHA256.to_string()));

    let backbone_path = ensure_model(
        app_handle,
        &models_dir,
        STYLE_TRANSFER_BACKBONE_FILENAME,
        "STYLE_TRANSFER_DINOV2_VITB",
        backbone_url.as_deref(),
        Some(STYLE_TRANSFER_BACKBONE_SHA256),
    )
    .await?;

    let preprocess_path = ensure_model(
        app_handle,
        &models_dir,
        STYLE_TRANSFER_PREPROCESS_FILENAME,
        "STYLE_TRANSFER_DINOV2_VITB_PREPROCESS",
        preprocess_url.as_deref(),
        preprocess_sha.as_deref(),
    )
    .await?;

    let preprocess_content = fs::read_to_string(preprocess_path)?;
    let preprocess: StyleTransferBackbonePreprocessConfig =
        serde_json::from_str(&preprocess_content)?;

    let _ = ort::init().with_name("AI-StyleTransfer").commit();
    let session = Session::builder()?.commit_from_file(backbone_path)?;

    crate::register_exit_handler();

    let models = Arc::new(StyleTransferBackboneModels {
        session: Mutex::new(session),
        preprocess,
    });

    let _ = STYLE_TRANSFER_BACKBONE_MODELS.set(models.clone());
    Ok(models)
}

pub fn extract_style_transfer_embedding(
    image: &DynamicImage,
    backbone_models: &StyleTransferBackboneModels,
) -> Result<Vec<f32>> {
    let input = preprocess_style_transfer_backbone_image(image, &backbone_models.preprocess);
    let input_dyn = input.into_dyn();
    let input_val = Tensor::from_array(input_dyn.as_standard_layout().into_owned())?;

    let output_dyn = {
        let mut session = backbone_models.session.lock().unwrap();
        let outputs = session.run(ort::inputs![input_val])?;
        outputs[0].try_extract_array::<f32>()?.to_owned()
    };

    let shape = output_dyn.shape().to_vec();
    let flat = output_dyn.into_raw_vec_and_offset().0;
    let embedding = match shape.as_slice() {
        [1, token_count, hidden_size] if *token_count > 0 && *hidden_size > 0 => {
            let hidden = *hidden_size;
            let tokens = *token_count;
            let cls = &flat[0..hidden];
            let mut patch_mean = vec![0.0f32; hidden];
            let patch_count = tokens.saturating_sub(1).max(1);
            for token_idx in 1..tokens {
                let base = token_idx * hidden;
                for channel_idx in 0..hidden {
                    patch_mean[channel_idx] += flat[base + channel_idx];
                }
            }
            for channel_idx in 0..hidden {
                patch_mean[channel_idx] /= patch_count as f32;
            }
            let mut fused = Vec::with_capacity(hidden * 2);
            fused.extend_from_slice(cls);
            fused.extend_from_slice(&patch_mean);
            fused
        }
        [1, hidden_size] if *hidden_size > 0 => flat,
        [hidden_size] if *hidden_size > 0 => flat,
        _ => {
            return Err(anyhow::anyhow!(
                "Unexpected style backbone output shape: {:?}",
                shape
            ));
        }
    };

    let norm = embedding
        .iter()
        .map(|v| (*v as f64) * (*v as f64))
        .sum::<f64>()
        .sqrt()
        .max(1e-12);
    Ok(embedding
        .into_iter()
        .map(|value| (value as f64 / norm) as f32)
        .collect())
}

pub async fn get_or_init_lama_model(
    app_handle: &tauri::AppHandle,
    ai_state_mutex: &Mutex<Option<AiState>>,
    ai_init_lock: &TokioMutex<()>,
) -> Result<Arc<Mutex<Session>>> {
    if let Some(lama_model) = ai_state_mutex
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|state| state.lama_model.clone())
    {
        return Ok(lama_model);
    }

    let _guard = ai_init_lock.lock().await;

    if let Some(lama_model) = ai_state_mutex
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|state| state.lama_model.clone())
    {
        return Ok(lama_model);
    }

    let models_dir = get_models_dir(app_handle)?;
    download_and_verify_model(
        app_handle,
        &models_dir,
        LAMA_FILENAME,
        LAMA_URL,
        LAMA_SHA256,
        "Inpainting Model",
    )
    .await?;

    let _ = ort::init().with_name("AI-Inpainting").commit();
    let model_path = models_dir.join(LAMA_FILENAME);
    let session = Session::builder()?.commit_from_file(model_path)?;
    let lama_model = Arc::new(Mutex::new(session));

    crate::register_exit_handler();

    let mut ai_state_lock = ai_state_mutex.lock().unwrap();
    if let Some(state) = ai_state_lock.as_mut() {
        state.lama_model = Some(lama_model.clone());
    } else {
        *ai_state_lock = Some(AiState {
            models: None,
            denoise_model: None,
            clip_models: None,
            lama_model: Some(lama_model.clone()),
            culling_models: None,
            embeddings: None,
            depth_map: None,
        });
    }

    Ok(lama_model)
}

pub async fn get_or_init_culling_models(
    app_handle: &tauri::AppHandle,
    ai_state_mutex: &Mutex<Option<AiState>>,
    ai_init_lock: &TokioMutex<()>,
) -> Result<Arc<CullingModels>> {
    if let Some(models) = ai_state_mutex
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|state| state.culling_models.clone())
    {
        return Ok(models);
    }

    let _guard = ai_init_lock.lock().await;

    if let Some(models) = ai_state_mutex
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|state| state.culling_models.clone())
    {
        return Ok(models);
    }

    let models_dir = get_qraw_models_dir()?;

    let face_url = get_model_env("FACE_DETECTOR", "URL");
    let face_sha = get_model_env("FACE_DETECTOR", "SHA256");
    let yunet_url =
        get_model_env("YUNET", "URL").or_else(|| Some(DEFAULT_CULL_YUNET_URL.to_string()));
    let yunet_sha =
        get_model_env("YUNET", "SHA256").or_else(|| Some(DEFAULT_CULL_YUNET_SHA256.to_string()));
    let expr_url = get_model_env("EXPRESSION", "URL")
        .or_else(|| Some(DEFAULT_CULL_EXPRESSION_URL.to_string()));
    let expr_sha = get_model_env("EXPRESSION", "SHA256")
        .or_else(|| Some(DEFAULT_CULL_EXPRESSION_SHA256.to_string()));
    let aes_url = get_model_env("AESTHETIC", "URL");
    let aes_sha = get_model_env("AESTHETIC", "SHA256");

    let face_path = ensure_model(
        app_handle,
        &models_dir,
        CULL_FACE_DETECTOR_FILENAME,
        "FACE_DETECTOR",
        face_url.as_deref(),
        face_sha.as_deref(),
    )
    .await?;
    let yunet_path = ensure_model(
        app_handle,
        &models_dir,
        CULL_YUNET_FILENAME,
        "YUNET",
        yunet_url.as_deref(),
        yunet_sha.as_deref(),
    )
    .await
    .ok();
    let expr_path = ensure_model(
        app_handle,
        &models_dir,
        CULL_EXPRESSION_FILENAME,
        "EXPRESSION",
        expr_url.as_deref(),
        expr_sha.as_deref(),
    )
    .await
    .ok();

    let aes_path = ensure_model(
        app_handle,
        &models_dir,
        CULL_AESTHETIC_FILENAME,
        "AESTHETIC",
        aes_url.as_deref(),
        aes_sha.as_deref(),
    )
    .await
    .ok();

    let _ = ort::init().with_name("AI-Culling").commit();

    let face_detector = Session::builder()?.commit_from_file(face_path)?;
    let yunet_detector = if let Some(p) = yunet_path {
        Session::builder()?.commit_from_file(p).ok().map(Mutex::new)
    } else {
        None
    };
    let expression_model = if let Some(p) = expr_path {
        Session::builder()?.commit_from_file(p).ok().map(Mutex::new)
    } else {
        None
    };
    let aesthetic_model = if let Some(p) = aes_path {
        Session::builder()?.commit_from_file(p).ok().map(Mutex::new)
    } else {
        None
    };

    crate::register_exit_handler();

    let models = Arc::new(CullingModels {
        face_detector: Mutex::new(face_detector),
        yunet_detector,
        expression_model,
        aesthetic_model,
    });

    let mut ai_state_lock = ai_state_mutex.lock().unwrap();
    if let Some(state) = ai_state_lock.as_mut() {
        state.culling_models = Some(models.clone());
    } else {
        *ai_state_lock = Some(AiState {
            models: None,
            denoise_model: None,
            clip_models: None,
            lama_model: None,
            culling_models: Some(models.clone()),
            embeddings: None,
            depth_map: None,
        });
    }

    Ok(models)
}

#[derive(Clone, Debug)]
pub struct FaceBox {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub score: f32,
}

#[derive(Clone, Debug)]
pub struct YunetFace {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub score: f32,
    pub landmarks: [(f32, f32); 5],
}

fn sigmoid(x: f32) -> f32 {
    1.0 / (1.0 + (-x).exp())
}

fn softmax_1d(xs: &[f32]) -> Vec<f32> {
    if xs.is_empty() {
        return Vec::new();
    }
    let m = xs.iter().cloned().fold(f32::NEG_INFINITY, |a, b| a.max(b));
    let mut exps = Vec::with_capacity(xs.len());
    let mut sum = 0.0f32;
    for &x in xs {
        let e = (x - m).exp();
        exps.push(e);
        sum += e;
    }
    if sum <= 0.0 {
        return vec![0.0; xs.len()];
    }
    exps.into_iter().map(|e| e / sum).collect()
}

fn iou(a: &FaceBox, b: &FaceBox) -> f32 {
    let x1 = a.x1.max(b.x1);
    let y1 = a.y1.max(b.y1);
    let x2 = a.x2.min(b.x2);
    let y2 = a.y2.min(b.y2);
    let w = (x2 - x1).max(0.0);
    let h = (y2 - y1).max(0.0);
    let inter = w * h;
    let area_a = (a.x2 - a.x1).max(0.0) * (a.y2 - a.y1).max(0.0);
    let area_b = (b.x2 - b.x1).max(0.0) * (b.y2 - b.y1).max(0.0);
    let union = area_a + area_b - inter;
    if union <= 0.0 { 0.0 } else { inter / union }
}

fn nms(mut boxes: Vec<FaceBox>, iou_thresh: f32) -> Vec<FaceBox> {
    boxes.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut kept: Vec<FaceBox> = Vec::new();
    for b in boxes {
        if kept.iter().all(|k| iou(k, &b) < iou_thresh) {
            kept.push(b);
        }
    }
    kept
}

fn iou_yunet(a: &YunetFace, b: &YunetFace) -> f32 {
    let x1 = a.x1.max(b.x1);
    let y1 = a.y1.max(b.y1);
    let x2 = a.x2.min(b.x2);
    let y2 = a.y2.min(b.y2);
    let w = (x2 - x1).max(0.0);
    let h = (y2 - y1).max(0.0);
    let inter = w * h;
    let area_a = (a.x2 - a.x1).max(0.0) * (a.y2 - a.y1).max(0.0);
    let area_b = (b.x2 - b.x1).max(0.0) * (b.y2 - b.y1).max(0.0);
    let union = area_a + area_b - inter;
    if union <= 0.0 { 0.0 } else { inter / union }
}

fn nms_yunet(mut faces: Vec<YunetFace>, iou_thresh: f32) -> Vec<YunetFace> {
    faces.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut kept: Vec<YunetFace> = Vec::new();
    for f in faces {
        if kept.iter().all(|k| iou_yunet(k, &f) < iou_thresh) {
            kept.push(f);
        }
    }
    kept
}

fn letterbox_rgb_nchw_0_1(image: &DynamicImage, size: u32) -> (Array4<f32>, f32, f32, f32) {
    let rgb = image.to_rgb8();
    let (w, h) = rgb.dimensions();
    let scale = (size as f32 / w as f32).min(size as f32 / h as f32);
    let new_w = (w as f32 * scale).round().max(1.0) as u32;
    let new_h = (h as f32 * scale).round().max(1.0) as u32;
    let resized = imageops::resize(&rgb, new_w, new_h, FilterType::Triangle);

    let mut canvas = ImageBuffer::from_pixel(size, size, Rgb([114u8, 114u8, 114u8]));
    let pad_x = ((size - new_w) / 2) as u32;
    let pad_y = ((size - new_h) / 2) as u32;
    imageops::replace(&mut canvas, &resized, pad_x as i64, pad_y as i64);

    let mut arr = Array4::<f32>::zeros((1, 3, size as usize, size as usize));
    for (x, y, p) in canvas.enumerate_pixels() {
        let xf = x as usize;
        let yf = y as usize;
        arr[[0, 0, yf, xf]] = p[0] as f32 / 255.0;
        arr[[0, 1, yf, xf]] = p[1] as f32 / 255.0;
        arr[[0, 2, yf, xf]] = p[2] as f32 / 255.0;
    }
    (arr, scale, pad_x as f32, pad_y as f32)
}

fn resize_rgb_nchw_0_255(image: &DynamicImage, w: u32, h: u32) -> Array4<f32> {
    let rgb = image.to_rgb8();
    let resized = imageops::resize(&rgb, w, h, FilterType::Triangle);
    let mut arr = Array4::<f32>::zeros((1, 3, h as usize, w as usize));
    for (x, y, p) in resized.enumerate_pixels() {
        let xf = x as usize;
        let yf = y as usize;
        arr[[0, 0, yf, xf]] = p[0] as f32;
        arr[[0, 1, yf, xf]] = p[1] as f32;
        arr[[0, 2, yf, xf]] = p[2] as f32;
    }
    arr
}

pub fn detect_faces_yolov8(
    image: &DynamicImage,
    face_detector_mutex: &Mutex<Session>,
) -> Result<Vec<FaceBox>> {
    let input_size: u32 = env::var("QRAW_FACE_INPUT_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(640);
    let conf_thresh: f32 = env::var("QRAW_FACE_CONF_THRESH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.25);
    let iou_thresh: f32 = env::var("QRAW_FACE_IOU_THRESH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.45);

    let (input, scale, pad_x, pad_y) = letterbox_rgb_nchw_0_1(image, input_size);

    let input_dyn = input.into_dyn();
    let input_val = Tensor::from_array(input_dyn.as_standard_layout().into_owned())?;

    let out_dyn = {
        let mut session = face_detector_mutex.lock().unwrap();
        let outputs = session.run(ort::inputs![input_val])?;
        outputs[0].try_extract_array::<f32>()?.to_owned()
    };
    let shape = out_dyn.shape().to_vec();
    let flat = out_dyn.into_raw_vec_and_offset().0;

    let (num_boxes, stride) = match shape.as_slice() {
        [1, n, m] => (*n, *m),
        [n, m] => (*n, *m),
        [m] => (1, *m),
        _ => return Ok(Vec::new()),
    };

    let (img_w, img_h) = image.dimensions();
    let mut boxes = Vec::new();
    for i in 0..num_boxes {
        let base = i * stride;
        if base + 4 >= flat.len() {
            break;
        }
        let x = flat[base + 0];
        let y = flat[base + 1];
        let w = flat[base + 2];
        let h = flat[base + 3];

        let conf = if stride == 5 {
            flat[base + 4]
        } else if stride >= 10 {
            flat[base + 4]
        } else if stride > 5 {
            let obj = flat[base + 4];
            let cls = flat[base + 5..base + stride]
                .iter()
                .cloned()
                .fold(f32::NEG_INFINITY, |a, b| a.max(b));
            obj * cls
        } else {
            continue;
        };

        let conf = if (0.0..=1.0).contains(&conf) {
            conf
        } else {
            sigmoid(conf)
        };
        if conf < conf_thresh {
            continue;
        }

        let x1_in = x - w / 2.0;
        let y1_in = y - h / 2.0;
        let x2_in = x + w / 2.0;
        let y2_in = y + h / 2.0;

        let x1 = ((x1_in - pad_x) / scale).clamp(0.0, img_w as f32);
        let y1 = ((y1_in - pad_y) / scale).clamp(0.0, img_h as f32);
        let x2 = ((x2_in - pad_x) / scale).clamp(0.0, img_w as f32);
        let y2 = ((y2_in - pad_y) / scale).clamp(0.0, img_h as f32);

        if x2 > x1 && y2 > y1 {
            boxes.push(FaceBox {
                x1,
                y1,
                x2,
                y2,
                score: conf,
            });
        }
    }

    Ok(nms(boxes, iou_thresh))
}

pub fn detect_faces_yunet(
    image: &DynamicImage,
    yunet_mutex: &Mutex<Session>,
) -> Result<Vec<YunetFace>> {
    let input_size: u32 = env::var("QRAW_YUNET_INPUT_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(320);
    let conf_thresh: f32 = env::var("QRAW_YUNET_CONF_THRESH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.6);
    let iou_thresh: f32 = env::var("QRAW_YUNET_IOU_THRESH")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0.3);

    let (img_w, img_h) = image.dimensions();
    if img_w == 0 || img_h == 0 {
        return Ok(Vec::new());
    }

    let input = resize_rgb_nchw_0_255(image, input_size, input_size);
    let input_dyn = input.into_dyn();
    let input_val = Tensor::from_array(input_dyn.as_standard_layout().into_owned())?;

    let out_dyn = {
        let mut session = yunet_mutex.lock().unwrap();
        let outputs = session.run(ort::inputs![input_val])?;
        outputs[0].try_extract_array::<f32>()?.to_owned()
    };
    let shape = out_dyn.shape().to_vec();
    let flat = out_dyn.into_raw_vec_and_offset().0;

    let (num, stride) = match shape.as_slice() {
        [1, n, m] => (*n, *m),
        [n, m] => (*n, *m),
        _ => return Ok(Vec::new()),
    };
    if stride < 15 {
        return Ok(Vec::new());
    }

    let sx = img_w as f32 / input_size as f32;
    let sy = img_h as f32 / input_size as f32;

    let mut faces: Vec<YunetFace> = Vec::new();
    for i in 0..num {
        let base = i * stride;
        if base + 14 >= flat.len() {
            break;
        }
        let x = flat[base + 0];
        let y = flat[base + 1];
        let w = flat[base + 2];
        let h = flat[base + 3];
        let score = flat[base + 4];
        if score < conf_thresh {
            continue;
        }
        let x1 = (x * sx).clamp(0.0, img_w as f32);
        let y1 = (y * sy).clamp(0.0, img_h as f32);
        let x2 = ((x + w) * sx).clamp(0.0, img_w as f32);
        let y2 = ((y + h) * sy).clamp(0.0, img_h as f32);
        if x2 <= x1 || y2 <= y1 {
            continue;
        }
        let mut lms = [(0.0f32, 0.0f32); 5];
        for k in 0..5 {
            let lx = flat[base + 5 + k * 2] * sx;
            let ly = flat[base + 6 + k * 2] * sy;
            lms[k] = (lx.clamp(0.0, img_w as f32), ly.clamp(0.0, img_h as f32));
        }
        faces.push(YunetFace {
            x1,
            y1,
            x2,
            y2,
            score,
            landmarks: lms,
        });
    }

    Ok(nms_yunet(faces, iou_thresh))
}

pub fn run_expression_model(
    face_crop: &DynamicImage,
    expression_mutex: &Mutex<Session>,
) -> Result<(f32, f32, f32)> {
    let gray = face_crop.to_luma8();
    let size = 64;
    let resized = imageops::resize(&gray, size, size, FilterType::Triangle);
    let mut arr = Array4::<f32>::zeros((1, 1, size as usize, size as usize));

    for (x, y, p) in resized.enumerate_pixels() {
        arr[[0, 0, y as usize, x as usize]] = p[0] as f32; // FERPlus uses 0-255 grayscale
    }

    let input_val = Tensor::from_array(arr.into_dyn().as_standard_layout().into_owned())?;

    let out_dyns = {
        let mut session = expression_mutex.lock().unwrap();
        match session.run(ort::inputs!["Input3" => input_val]) {
            Ok(outputs) => {
                let mut extracted: Vec<ndarray::ArrayD<f32>> = Vec::new();
                for out in outputs.iter() {
                    if let Ok(arr) = out.1.try_extract_array::<f32>() {
                        extracted.push(arr.to_owned());
                    }
                }
                extracted
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    };

    if out_dyns.is_empty() {
        return Ok((0.0, 0.5, 0.0));
    }

    let flat = out_dyns[0].clone().into_raw_vec_and_offset().0;
    if flat.len() < 8 {
        return Ok((0.0, 0.5, 0.0));
    }

    // Apply softmax to get probabilities
    let probs = softmax_1d(&flat[..8]);

    // 0: neutral, 1: happiness, 2: surprise, 3: sadness, 4: anger, 5: disgust, 6: fear, 7: contempt
    let smile_prob = probs[1];

    // Define unnatural as negative emotions that look awkward in typical portrait photography
    let mut unnatural_prob = probs[3] + probs[4] + probs[5] + probs[6] + probs[7];

    // High entropy = ambiguous expression (e.g., mid-speech, awkward transition)
    let mut entropy = 0.0;
    for p in &probs {
        if *p > 1e-6 {
            entropy -= p * p.ln();
        }
    }

    // Max entropy for 8 classes is ln(8) ≈ 2.079.
    // If entropy > 1.2, it's very ambiguous.
    if entropy > 1.2 && !entropy.is_nan() {
        unnatural_prob += (entropy - 1.2) * 0.5;
    }

    // Add surprise penalty if it's awkward (surprise but no smile/happiness)
    if probs[2] > 0.4 && probs[1] < 0.2 {
        unnatural_prob += probs[2] * 0.5;
    }

    if unnatural_prob.is_nan() {
        unnatural_prob = 0.0;
    }
    unnatural_prob = unnatural_prob.clamp(0.0, 1.0);

    // We cannot detect blink with this model. Fallback to 0.
    let blink_prob = 0.0;

    Ok((blink_prob, smile_prob, unnatural_prob))
}

pub fn score_aesthetics_nima(
    image: &DynamicImage,
    aesthetic_mutex: &Mutex<Session>,
) -> Result<f32> {
    let size: u32 = env::var("QRAW_NIMA_INPUT_SIZE")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(224);

    let rgb = image.to_rgb8();
    let resized = imageops::resize(&rgb, size, size, FilterType::Triangle);
    let mean = [0.485f32, 0.456f32, 0.406f32];
    let std = [0.229f32, 0.224f32, 0.225f32];

    let mut arr = Array4::<f32>::zeros((1, 3, size as usize, size as usize));
    for (x, y, p) in resized.enumerate_pixels() {
        let xf = x as usize;
        let yf = y as usize;
        let r = (p[0] as f32 / 255.0 - mean[0]) / std[0];
        let g = (p[1] as f32 / 255.0 - mean[1]) / std[1];
        let b = (p[2] as f32 / 255.0 - mean[2]) / std[2];
        arr[[0, 0, yf, xf]] = r;
        arr[[0, 1, yf, xf]] = g;
        arr[[0, 2, yf, xf]] = b;
    }

    let input_val = Tensor::from_array(arr.into_dyn().as_standard_layout().into_owned())?;
    let out_dyn = {
        let mut session = aesthetic_mutex.lock().unwrap();
        let outputs = session.run(ort::inputs![input_val])?;
        outputs[0].try_extract_array::<f32>()?.to_owned()
    };
    let flat = out_dyn.into_raw_vec_and_offset().0;
    if flat.len() < 10 {
        return Ok(5.0);
    }
    let probs = if flat.iter().all(|v| (0.0..=1.0).contains(v)) {
        flat[..10].to_vec()
    } else {
        softmax_1d(&flat[..10])
    };
    let mut mean_score = 0.0f32;
    for (i, p) in probs.iter().enumerate() {
        mean_score += (i as f32 + 1.0) * p;
    }
    Ok(mean_score.clamp(1.0, 10.0))
}

#[derive(Clone, Copy)]
struct TileParams {
    cs: usize,
    ucs: usize,
    overlap: usize,
    pad: usize,
}

impl TileParams {
    const fn new(cs: usize, ucs: usize, overlap: usize) -> Self {
        Self {
            cs,
            ucs,
            overlap,
            pad: (cs - ucs) / 2,
        }
    }
}

const TILE_BALANCED: TileParams = TileParams::new(504, 480, 6);
const TILE_FASTER: TileParams = TileParams::new(504, 504, 0);
const TILE_HIGHER_QUALITY: TileParams = TileParams::new(504, 448, 12);

fn select_tile_params(quality_0_1: f32) -> TileParams {
    let q = quality_0_1.clamp(0.0, 1.0);
    if q <= 0.25 {
        TILE_FASTER
    } else if q >= 0.75 {
        TILE_HIGHER_QUALITY
    } else {
        TILE_BALANCED
    }
}

#[inline]
fn mirror_coord(c: i32, size: i32) -> i32 {
    if c < 0 {
        (-c).min(size - 1)
    } else if c >= size {
        (2 * size - 1 - c).max(0)
    } else {
        c
    }
}

fn extract_tile_mirror(img: &Rgb32FImage, x0: i32, y0: i32, cs: usize) -> Array4<f32> {
    let (w, h) = (img.width() as i32, img.height() as i32);
    let mut arr = Array4::zeros((1, 3, cs, cs));
    for dy in 0..cs as i32 {
        for dx in 0..cs as i32 {
            let sx = mirror_coord(x0 + dx, w);
            let sy = mirror_coord(y0 + dy, h);
            let px = img.get_pixel(sx as u32, sy as u32);
            arr[[0, 0, dy as usize, dx as usize]] = px[0];
            arr[[0, 1, dy as usize, dx as usize]] = px[1];
            arr[[0, 2, dy as usize, dx as usize]] = px[2];
        }
    }
    arr
}

struct SeamlessBlend {
    ud0: usize,
    ud1: usize,
    ud2: usize,
    ud3: usize,
    absx0: usize,
    absy0: usize,
    fswidth: usize,
    fsheight: usize,
    overlap: usize,
}

fn apply_seamless(tile: &mut Array4<f32>, blend: &SeamlessBlend) {
    let SeamlessBlend {
        ud0,
        ud1,
        ud2,
        ud3,
        absx0,
        absy0,
        fswidth,
        fsheight,
        overlap,
    } = *blend;
    let ol = overlap;
    if absx0 > 0 {
        for c in 0..3 {
            for y in ud1..ud3 {
                for x in ud0..(ud0 + ol).min(ud2) {
                    tile[[0, c, y, x]] *= 0.5;
                }
            }
        }
    }
    if absy0 > 0 {
        for c in 0..3 {
            for y in ud1..(ud1 + ol).min(ud3) {
                for x in ud0..ud2 {
                    tile[[0, c, y, x]] *= 0.5;
                }
            }
        }
    }
    if absx0 + (ud2 - ud0) < fswidth && ol > 0 {
        let right_start = (ud2 as i32 - ol as i32).max(ud0 as i32) as usize;
        for c in 0..3 {
            for y in ud1..ud3 {
                for x in right_start..ud2 {
                    tile[[0, c, y, x]] *= 0.5;
                }
            }
        }
    }
    if absy0 + (ud3 - ud1) < fsheight && ol > 0 {
        let bottom_start = (ud3 as i32 - ol as i32).max(ud1 as i32) as usize;
        for c in 0..3 {
            for y in bottom_start..ud3 {
                for x in ud0..ud2 {
                    tile[[0, c, y, x]] *= 0.5;
                }
            }
        }
    }
}

fn run_native_denoise(
    img: &Rgb32FImage,
    session: &Mutex<Session>,
    accumulator: &mut [f32],
    width: usize,
    height: usize,
    app_handle: &tauri::AppHandle,
    params: TileParams,
) -> Result<()> {
    let w = width as i32;
    let h = height as i32;
    let step = params.ucs.saturating_sub(params.overlap).max(1);
    let iperhl = (width.saturating_sub(params.ucs) as f64 / step as f64).ceil() as usize;
    let ipervl = (height.saturating_sub(params.ucs) as f64 / step as f64).ceil() as usize;
    let total = (iperhl + 1) * (ipervl + 1);

    for i in 0..total {
        let yi = i / (iperhl + 1);
        let xi = i % (iperhl + 1);
        let x0 =
            params.ucs as i32 * xi as i32 - params.overlap as i32 * xi as i32 - params.pad as i32;
        let y0 =
            params.ucs as i32 * yi as i32 - params.overlap as i32 * yi as i32 - params.pad as i32;

        if i % 10 == 0 {
            let pct = (i as f32 / total as f32) * 100.0;
            let _ = app_handle.emit("denoise-progress", format!("Denoising… {:.0}%", pct));
        }

        let crop = extract_tile_mirror(img, x0, y0, params.cs);
        let input_values = crop.as_standard_layout().to_owned();
        let t_input = Tensor::from_array(input_values)?;

        let out = {
            let mut sess = session.lock().unwrap();
            let outputs = sess.run(ort::inputs![t_input])?;
            let arr = outputs[0].try_extract_array::<f32>()?.to_owned();
            arr.into_dimensionality::<ndarray::Ix4>()
                .map_err(|e| anyhow::anyhow!("Unexpected output shape: {}", e))?
        };

        let x1pad = (0i32).max(x0 + params.cs as i32 - w) as usize;
        let y1pad = (0i32).max(y0 + params.cs as i32 - h) as usize;
        let ud0 = params.pad;
        let ud1 = params.pad;
        let ud2 = params.cs - params.pad.max(x1pad);
        let ud3 = params.cs - params.pad.max(y1pad);
        let absx0 = (x0 + params.pad as i32).max(0) as usize;
        let absy0 = (y0 + params.pad as i32).max(0) as usize;

        let mut tile = out;
        apply_seamless(
            &mut tile,
            &SeamlessBlend {
                ud0,
                ud1,
                ud2,
                ud3,
                absx0,
                absy0,
                fswidth: width,
                fsheight: height,
                overlap: params.overlap,
            },
        );

        for cy in 0..(ud3 - ud1) {
            for cx in 0..(ud2 - ud0) {
                let gx = absx0 + cx;
                let gy = absy0 + cy;
                if gx < width && gy < height {
                    let base = (gy * width + gx) * 3;
                    accumulator[base] += tile[[0, 0, ud1 + cy, ud0 + cx]].clamp(0.0, 1.0);
                    accumulator[base + 1] += tile[[0, 1, ud1 + cy, ud0 + cx]].clamp(0.0, 1.0);
                    accumulator[base + 2] += tile[[0, 2, ud1 + cy, ud0 + cx]].clamp(0.0, 1.0);
                }
            }
        }
    }
    Ok(())
}

fn accumulator_to_rgb32f(acc: &[f32], width: u32, height: u32) -> Rgb32FImage {
    let mut out = Rgb32FImage::new(width, height);
    for (i, p) in out.pixels_mut().enumerate() {
        let i3 = i * 3;
        *p = Rgb([
            acc[i3].clamp(0.0, 1.0),
            acc[i3 + 1].clamp(0.0, 1.0),
            acc[i3 + 2].clamp(0.0, 1.0),
        ]);
    }
    out
}

pub fn run_ai_denoise(
    rgb_img: &Rgb32FImage,
    intensity: f32,
    session: &Mutex<Session>,
    app_handle: &tauri::AppHandle,
) -> Result<DynamicImage> {
    let (width, height) = rgb_img.dimensions();
    let params = select_tile_params(intensity);

    let _ = app_handle.emit("denoise-progress", "Denoising (AI NIND)...");
    let mut accumulator = vec![0.0f32; width as usize * height as usize * 3];
    run_native_denoise(
        rgb_img,
        session,
        &mut accumulator,
        width as usize,
        height as usize,
        app_handle,
        params,
    )?;

    let out_img_buffer = accumulator_to_rgb32f(&accumulator, width, height);
    Ok(DynamicImage::ImageRgb32F(out_img_buffer))
}

pub fn run_lama_inpainting(
    image: &DynamicImage,
    mask: &GrayImage,
    lama_session: &Mutex<Session>,
) -> Result<RgbaImage> {
    let (w, h) = image.dimensions();

    let (mut min_x, mut min_y) = (w, h);
    let (mut max_x, mut max_y) = (0u32, 0u32);
    let mut has_mask = false;

    for (x, y, p) in mask.enumerate_pixels() {
        if p[0] > 0 {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            has_mask = true;
        }
    }

    if !has_mask {
        return Ok(image.to_rgba8());
    }

    let mask_w = max_x - min_x + 1;
    let mask_h = max_y - min_y + 1;

    let pad_x = 128.max((mask_w as f32 * 1.5) as u32);
    let pad_y = 128.max((mask_h as f32 * 1.5) as u32);

    let x0 = min_x.saturating_sub(pad_x);
    let y0 = min_y.saturating_sub(pad_y);
    let x1 = (max_x + pad_x).min(w.saturating_sub(1));
    let y1 = (max_y + pad_y).min(h.saturating_sub(1));

    let crop_w = x1 - x0 + 1;
    let crop_h = y1 - y0 + 1;

    let rgba = image.to_rgba8();

    let cropped_img = imageops::crop_imm(&rgba, x0, y0, crop_w, crop_h).to_image();
    let cropped_mask = imageops::crop_imm(mask, x0, y0, crop_w, crop_h).to_image();

    let max_dim_limit: u32 = 768;
    let needs_downscale = crop_w > max_dim_limit || crop_h > max_dim_limit;

    let (fw, fh, inf_img, inf_mask) = if needs_downscale {
        let scale = max_dim_limit as f32 / crop_w.max(crop_h) as f32;

        let scaled_w = (crop_w as f32 * scale).round().max(1.0) as u32;
        let scaled_h = (crop_h as f32 * scale).round().max(1.0) as u32;

        (
            scaled_w,
            scaled_h,
            imageops::resize(&cropped_img, scaled_w, scaled_h, FilterType::Lanczos3),
            imageops::resize(&cropped_mask, scaled_w, scaled_h, FilterType::Triangle),
        )
    } else {
        (crop_w, crop_h, cropped_img.clone(), cropped_mask.clone())
    };

    let align = 64u32;
    let mut tensor_dim = fw.max(fh);
    if tensor_dim % align != 0 {
        tensor_dim += align - (tensor_dim % align);
    }
    let tensor_dim = tensor_dim.max(align) as usize;

    let mut img_tensor = Array::<f32, _>::zeros((1, 3, tensor_dim, tensor_dim));
    let mut msk_tensor = Array::<f32, _>::zeros((1, 1, tensor_dim, tensor_dim));

    for y in 0..tensor_dim {
        for x in 0..tensor_dim {
            let sx = (x as u32).min(fw.saturating_sub(1));
            let sy = (y as u32).min(fh.saturating_sub(1));

            let p = inf_img.get_pixel(sx, sy);
            let m = inf_mask.get_pixel(sx, sy)[0];

            img_tensor[[0, 0, y, x]] = p[0] as f32 / 255.0;
            img_tensor[[0, 1, y, x]] = p[1] as f32 / 255.0;
            img_tensor[[0, 2, y, x]] = p[2] as f32 / 255.0;
            msk_tensor[[0, 0, y, x]] = if m > 0 { 1.0 } else { 0.0 };
        }
    }

    let t_img = Tensor::from_array(img_tensor.into_dyn().as_standard_layout().into_owned())?;
    let t_msk = Tensor::from_array(msk_tensor.into_dyn().as_standard_layout().into_owned())?;

    let output_tensor = {
        let mut session = lama_session.lock().unwrap();
        let outputs = session.run(ort::inputs!["image" => t_img, "mask" => t_msk])?;
        outputs[0].try_extract_array::<f32>()?.to_owned()
    };

    let mut result_inf = RgbaImage::new(fw, fh);
    for y in 0..fh {
        for x in 0..fw {
            let r = output_tensor[[0, 0, y as usize, x as usize]].clamp(0.0, 255.0) as u8;
            let g = output_tensor[[0, 1, y as usize, x as usize]].clamp(0.0, 255.0) as u8;
            let b = output_tensor[[0, 2, y as usize, x as usize]].clamp(0.0, 255.0) as u8;
            result_inf.put_pixel(x, y, Rgba([r, g, b, 255]));
        }
    }

    let result_crop = if needs_downscale {
        imageops::resize(&result_inf, crop_w, crop_h, FilterType::Lanczos3)
    } else {
        result_inf
    };

    let mut final_image = image.to_rgba8();

    for y in 0..crop_h {
        for x in 0..crop_w {
            let m = cropped_mask.get_pixel(x, y)[0];
            if m > 0 {
                let alpha = m as f32 / 255.0;
                let p = result_crop.get_pixel(x, y);
                let gx = x0 + x;
                let gy = y0 + y;
                let orig = final_image.get_pixel(gx, gy);

                let r = (p[0] as f32 * alpha + orig[0] as f32 * (1.0 - alpha)) as u8;
                let g = (p[1] as f32 * alpha + orig[1] as f32 * (1.0 - alpha)) as u8;
                let b = (p[2] as f32 * alpha + orig[2] as f32 * (1.0 - alpha)) as u8;

                final_image.put_pixel(gx, gy, Rgba([r, g, b, 255]));
            }
        }
    }

    Ok(final_image)
}

pub fn generate_image_embeddings(
    image: &DynamicImage,
    encoder: &Mutex<Session>,
) -> Result<ImageEmbeddings> {
    let (orig_width, orig_height) = image.dimensions();

    let long_side = orig_width.max(orig_height) as f32;
    let scale = SAM_INPUT_SIZE as f32 / long_side;
    let new_width = (orig_width as f32 * scale).round() as u32;
    let new_height = (orig_height as f32 * scale).round() as u32;

    let resized_image = image.resize(new_width, new_height, FilterType::Triangle);
    let rgb_image = resized_image.into_rgb8();
    let (actual_width, actual_height) = rgb_image.dimensions();
    let raw_pixels = rgb_image.as_raw();

    let mut input_tensor: Array<u8, _> =
        Array::zeros((1, 3, SAM_INPUT_SIZE as usize, SAM_INPUT_SIZE as usize));

    let w_usize = actual_width as usize;
    for y in 0..(actual_height as usize) {
        for x in 0..w_usize {
            let idx = (y * w_usize + x) * 3;
            input_tensor[[0, 0, y, x]] = raw_pixels[idx];
            input_tensor[[0, 1, y, x]] = raw_pixels[idx + 1];
            input_tensor[[0, 2, y, x]] = raw_pixels[idx + 2];
        }
    }

    let input_tensor_dyn = input_tensor.into_dyn();
    let input_values = input_tensor_dyn.as_standard_layout();
    let input_tensor_ort = Tensor::from_array(input_values.into_owned())?;
    let mut session = encoder.lock().unwrap();
    let outputs = session.run(ort::inputs![input_tensor_ort])?;

    let embeddings = outputs[0].try_extract_array::<f32>()?.to_owned();

    Ok(ImageEmbeddings {
        path_hash: "".to_string(),
        embeddings: embeddings.into_dyn(),
        original_size: (orig_width, orig_height),
    })
}

pub fn run_sam_decoder(
    decoder: &Mutex<Session>,
    embeddings: &ImageEmbeddings,
    start_point: (f64, f64),
    end_point: (f64, f64),
) -> Result<GrayImage> {
    let (orig_width, orig_height) = embeddings.original_size;
    let long_side = orig_width.max(orig_height) as f64;
    let scale = SAM_INPUT_SIZE as f64 / long_side;

    let iters = 2;

    let is_point =
        (start_point.0 - end_point.0).abs() < 1e-6 && (start_point.1 - end_point.1).abs() < 1e-6;
    let mut point_coords = Vec::new();
    let mut point_labels = Vec::new();

    if is_point {
        point_coords.push((
            (start_point.0 * scale) as f32,
            (start_point.1 * scale) as f32,
        ));
        point_labels.push(1.0f32);
    } else {
        let x1 = (start_point.0.min(end_point.0) * scale) as f32;
        let y1 = (start_point.1.min(end_point.1) * scale) as f32;
        let x2 = (start_point.0.max(end_point.0) * scale) as f32;
        let y2 = (start_point.1.max(end_point.1) * scale) as f32;
        point_coords.push((x1, y1));
        point_coords.push((x2, y2));
        point_labels.push(2.0f32);
        point_labels.push(3.0f32);
    }

    let mut mask_input = Array::zeros((1, 1, 256, 256)).into_dyn();
    let mut has_mask_input = 0.0f32;

    let orig_im_size =
        Array::from_shape_vec((2,), vec![orig_height as f32, orig_width as f32])?.into_dyn();

    let mut final_mask_data: Vec<u8> = Vec::new();
    let mut final_w = 0;
    let mut final_h = 0;

    for i in 0..iters {
        let pc_len = point_coords.len();
        let pl_len = point_labels.len();

        let coords_flat: Vec<f32> = point_coords.iter().flat_map(|&(x, y)| vec![x, y]).collect();
        let coords_array = Array::from_shape_vec((1, pc_len, 2), coords_flat)?.into_dyn();
        let labels_array = Array::from_shape_vec((1, pl_len), point_labels.clone())?.into_dyn();

        let t_embeddings = Tensor::from_array(
            embeddings
                .embeddings
                .clone()
                .as_standard_layout()
                .into_owned(),
        )?;
        let t_point_coords = Tensor::from_array(coords_array.as_standard_layout().into_owned())?;
        let t_point_labels = Tensor::from_array(labels_array.as_standard_layout().into_owned())?;
        let t_mask_input =
            Tensor::from_array(mask_input.clone().as_standard_layout().into_owned())?;
        let t_has_mask = Tensor::from_array(
            Array::from_elem((1,), has_mask_input)
                .into_dyn()
                .as_standard_layout()
                .into_owned(),
        )?;
        let t_orig_im_size =
            Tensor::from_array(orig_im_size.clone().as_standard_layout().into_owned())?;

        let mask_tensor = {
            let mut session = decoder.lock().unwrap();
            let outputs = session.run(ort::inputs![
                t_embeddings,
                t_point_coords,
                t_point_labels,
                t_mask_input,
                t_has_mask,
                t_orig_im_size
            ])?;
            outputs[0].try_extract_array::<f32>()?.to_owned()
        };

        let mask_dims = mask_tensor.shape();
        let h = mask_dims[2];
        let w = mask_dims[3];
        let area = h * w;

        let mask_slice = mask_tensor.as_slice().unwrap();
        let first_mask_slice = &mask_slice[0..area];

        if i == iters - 1 {
            final_mask_data = first_mask_slice
                .iter()
                .map(|&val| if val > 0.0 { 255 } else { 0 })
                .collect();
            final_w = w;
            final_h = h;
            break;
        }

        let mut binary_mask = vec![false; area];
        let mut mask_area = 0.0;
        let mut min_x = w;
        let mut min_y = h;
        let mut max_x = 0;
        let mut max_y = 0;

        for (idx, &val) in first_mask_slice.iter().enumerate() {
            if val > 0.0 {
                binary_mask[idx] = true;
                let x = idx % w;
                let y = idx / w;
                min_x = min_x.min(x);
                max_x = max_x.max(x);
                min_y = min_y.min(y);
                max_y = max_y.max(y);
                mask_area += 1.0;
            }
        }

        if mask_area == 0.0 || min_x > max_x {
            final_mask_data = first_mask_slice
                .iter()
                .map(|&val| if val > 0.0 { 255 } else { 0 })
                .collect();
            final_w = w;
            final_h = h;
            break;
        }

        let dt_in = edt_2d(&binary_mask, w, h);
        let mut max_in = 0.0;
        let mut pos_idx = 0;
        for (idx, &v) in dt_in.iter().enumerate() {
            if v > max_in {
                max_in = v;
                pos_idx = idx;
            }
        }
        let pos_y = pos_idx / w;
        let pos_x = pos_idx % w;

        let mut rev_mask = vec![false; area];
        for (idx, is_true) in binary_mask.iter().enumerate() {
            rev_mask[idx] = !is_true;
        }
        let mut dt_out = edt_2d(&rev_mask, w, h);

        for y in 0..h {
            for x in 0..w {
                if x < min_x || x > max_x || y < min_y || y > max_y {
                    dt_out[y * w + x] = 0.0;
                }
            }
        }

        let mut max_out = 0.0;
        let mut neg_idx = 0;
        for (idx, &v) in dt_out.iter().enumerate() {
            if v > max_out {
                max_out = v;
                neg_idx = idx;
            }
        }
        let neg_y = neg_idx / w;
        let neg_x = neg_idx % w;

        point_coords.clear();
        point_labels.clear();

        point_coords.push(((pos_x as f64 * scale) as f32, (pos_y as f64 * scale) as f32));
        point_labels.push(1.0);
        point_coords.push(((neg_x as f64 * scale) as f32, (neg_y as f64 * scale) as f32));
        point_labels.push(0.0);
        point_coords.push(((min_x as f64 * scale) as f32, (min_y as f64 * scale) as f32));
        point_labels.push(2.0);
        point_coords.push(((max_x as f64 * scale) as f32, (max_y as f64 * scale) as f32));
        point_labels.push(3.0);

        let mut gaus_dt = vec![0.0f32; area];
        let variance = (mask_area / 4.0_f32).max(1.0_f32);
        for (idx, &is_true) in binary_mask.iter().enumerate() {
            if is_true {
                let diff = dt_in[idx] - max_in;
                gaus_dt[idx] = (-(diff * diff) / variance).exp();
            }
        }

        let mask_f32_vec: Vec<f32> = first_mask_slice
            .iter()
            .map(|&v| if v > 0.0 { 15.0 } else { -15.0 })
            .collect();

        let img_mask_f32 =
            ImageBuffer::<Luma<f32>, Vec<f32>>::from_raw(w as u32, h as u32, mask_f32_vec).unwrap();
        let img_gaus_f32 =
            ImageBuffer::<Luma<f32>, Vec<f32>>::from_raw(w as u32, h as u32, gaus_dt).unwrap();

        let resized_mask = imageops::resize(&img_mask_f32, 256, 256, FilterType::Triangle);
        let resized_gaus = imageops::resize(&img_gaus_f32, 256, 256, FilterType::Triangle);

        let rm_raw = resized_mask.as_raw();
        let rg_raw = resized_gaus.as_raw();
        let mut mask_input_flat = vec![0.0f32; 256 * 256];

        for i in 0..(256 * 256) {
            let m_val = rm_raw[i];
            let mut g_val = rg_raw[i];
            if g_val <= 0.0 {
                g_val = 1.0;
            }
            mask_input_flat[i] = m_val * g_val;
        }

        mask_input = Array::from_shape_vec((1, 1, 256, 256), mask_input_flat)
            .unwrap()
            .into_dyn();
        has_mask_input = 1.0;
    }

    let gray_mask = GrayImage::from_raw(final_w as u32, final_h as u32, final_mask_data)
        .ok_or_else(|| anyhow::anyhow!("Failed to create mask image from raw data"))?;

    let feathered_mask = image::imageops::blur(&gray_mask, 2.0);

    Ok(feathered_mask)
}

pub fn run_sky_seg_model(
    image: &DynamicImage,
    sky_seg_session: &Mutex<Session>,
) -> Result<GrayImage> {
    let (orig_width, orig_height) = image.dimensions();

    let resized_image = image.resize(SKYSEG_INPUT_SIZE, SKYSEG_INPUT_SIZE, FilterType::Triangle);
    let (resized_w, resized_h) = resized_image.dimensions();
    let resized_rgb = resized_image.into_rgb8();
    let raw_pixels = resized_rgb.as_raw();

    let paste_x = ((SKYSEG_INPUT_SIZE - resized_w) / 2) as usize;
    let paste_y = ((SKYSEG_INPUT_SIZE - resized_h) / 2) as usize;

    let mut input_tensor: Array<f32, _> =
        Array::zeros((1, 3, SKYSEG_INPUT_SIZE as usize, SKYSEG_INPUT_SIZE as usize));

    let mean = [0.485, 0.456, 0.406];
    let std = [0.229, 0.224, 0.225];

    let rw = resized_w as usize;
    let rh = resized_h as usize;

    for y in 0..rh {
        for x in 0..rw {
            let idx = (y * rw + x) * 3;
            let dest_y = y + paste_y;
            let dest_x = x + paste_x;

            input_tensor[[0, 0, dest_y, dest_x]] =
                (raw_pixels[idx] as f32 / 255.0 - mean[0]) / std[0];
            input_tensor[[0, 1, dest_y, dest_x]] =
                (raw_pixels[idx + 1] as f32 / 255.0 - mean[1]) / std[1];
            input_tensor[[0, 2, dest_y, dest_x]] =
                (raw_pixels[idx + 2] as f32 / 255.0 - mean[2]) / std[2];
        }
    }

    let input_tensor_dyn = input_tensor.into_dyn();
    let t_input = Tensor::from_array(input_tensor_dyn.as_standard_layout().into_owned())?;

    let mut session = sky_seg_session.lock().unwrap();
    let outputs = session.run(ort::inputs![t_input])?;
    let output_tensor = outputs[0].try_extract_array::<f32>()?.to_owned();
    let out_slice = output_tensor.as_slice().unwrap();

    let mut min_val = f32::MAX;
    let mut max_val = f32::MIN;
    for &v in out_slice {
        min_val = min_val.min(v);
        max_val = max_val.max(v);
    }

    let range = max_val - min_val;
    let scale = if range > 1e-6 { 255.0 / range } else { 0.0 };

    let usize_size = SKYSEG_INPUT_SIZE as usize;
    let mut cropped_mask_data = Vec::with_capacity(rw * rh);

    for y in 0..rh {
        let src_y = y + paste_y;
        for x in 0..rw {
            let src_x = x + paste_x;
            let val = out_slice[src_y * usize_size + src_x];
            let pixel = if range > 1e-6 {
                ((val - min_val) * scale) as u8
            } else {
                0
            };
            cropped_mask_data.push(pixel);
        }
    }

    let cropped_mask = GrayImage::from_raw(resized_w, resized_h, cropped_mask_data)
        .ok_or_else(|| anyhow::anyhow!("Failed to create mask from Sky Segmentation output"))?;

    let final_mask = imageops::resize(&cropped_mask, orig_width, orig_height, FilterType::Triangle);

    Ok(final_mask)
}

pub fn run_u2netp_model(
    image: &DynamicImage,
    u2netp_session: &Mutex<Session>,
) -> Result<GrayImage> {
    let (orig_width, orig_height) = image.dimensions();

    let resized_image = image.resize(U2NETP_INPUT_SIZE, U2NETP_INPUT_SIZE, FilterType::Triangle);
    let (resized_w, resized_h) = resized_image.dimensions();
    let resized_rgb = resized_image.into_rgb8();
    let raw_pixels = resized_rgb.as_raw();

    let paste_x = ((U2NETP_INPUT_SIZE - resized_w) / 2) as usize;
    let paste_y = ((U2NETP_INPUT_SIZE - resized_h) / 2) as usize;

    let mut input_tensor: Array<f32, _> =
        Array::zeros((1, 3, U2NETP_INPUT_SIZE as usize, U2NETP_INPUT_SIZE as usize));

    let mean = [0.485, 0.456, 0.406];
    let std = [0.229, 0.224, 0.225];

    let rw = resized_w as usize;
    let rh = resized_h as usize;

    for y in 0..rh {
        for x in 0..rw {
            let idx = (y * rw + x) * 3;
            let dest_y = y + paste_y;
            let dest_x = x + paste_x;

            input_tensor[[0, 0, dest_y, dest_x]] =
                (raw_pixels[idx] as f32 / 255.0 - mean[0]) / std[0];
            input_tensor[[0, 1, dest_y, dest_x]] =
                (raw_pixels[idx + 1] as f32 / 255.0 - mean[1]) / std[1];
            input_tensor[[0, 2, dest_y, dest_x]] =
                (raw_pixels[idx + 2] as f32 / 255.0 - mean[2]) / std[2];
        }
    }

    let input_tensor_dyn = input_tensor.into_dyn();
    let t_input = Tensor::from_array(input_tensor_dyn.as_standard_layout().into_owned())?;

    let mut session = u2netp_session.lock().unwrap();
    let outputs = session.run(ort::inputs![t_input])?;
    let output_tensor = outputs[0].try_extract_array::<f32>()?.to_owned();
    let out_slice = output_tensor.as_slice().unwrap();

    let mut min_val = f32::MAX;
    let mut max_val = f32::MIN;
    for &v in out_slice {
        min_val = min_val.min(v);
        max_val = max_val.max(v);
    }

    let range = max_val - min_val;
    let scale = if range > 1e-6 { 255.0 / range } else { 0.0 };

    let usize_size = U2NETP_INPUT_SIZE as usize;
    let mut cropped_mask_data = Vec::with_capacity(rw * rh);

    for y in 0..rh {
        let src_y = y + paste_y;
        for x in 0..rw {
            let src_x = x + paste_x;
            let val = out_slice[src_y * usize_size + src_x];
            let pixel = if range > 1e-6 {
                ((val - min_val) * scale) as u8
            } else {
                0
            };
            cropped_mask_data.push(pixel);
        }
    }

    let cropped_mask = GrayImage::from_raw(resized_w, resized_h, cropped_mask_data)
        .ok_or_else(|| anyhow::anyhow!("Failed to create mask from U-2-Netp output"))?;

    let final_mask = imageops::resize(&cropped_mask, orig_width, orig_height, FilterType::Triangle);

    Ok(final_mask)
}

pub fn run_depth_anything_model(
    image: &DynamicImage,
    depth_session: &Mutex<Session>,
) -> Result<GrayImage> {
    let resized_image = image.resize(DEPTH_INPUT_SIZE, DEPTH_INPUT_SIZE, FilterType::Triangle);
    let (resized_w, resized_h) = resized_image.dimensions();
    let resized_rgb = resized_image.into_rgb8();
    let raw_pixels = resized_rgb.as_raw();

    let paste_x = ((DEPTH_INPUT_SIZE - resized_w) / 2) as usize;
    let paste_y = ((DEPTH_INPUT_SIZE - resized_h) / 2) as usize;

    let mut input_tensor: Array<f32, _> =
        Array::zeros((1, 3, DEPTH_INPUT_SIZE as usize, DEPTH_INPUT_SIZE as usize));

    let mean = [0.485, 0.456, 0.406];
    let std = [0.229, 0.224, 0.225];

    let rw = resized_w as usize;
    let rh = resized_h as usize;

    for y in 0..rh {
        for x in 0..rw {
            let idx = (y * rw + x) * 3;
            let dest_y = y + paste_y;
            let dest_x = x + paste_x;

            input_tensor[[0, 0, dest_y, dest_x]] =
                (raw_pixels[idx] as f32 / 255.0 - mean[0]) / std[0];
            input_tensor[[0, 1, dest_y, dest_x]] =
                (raw_pixels[idx + 1] as f32 / 255.0 - mean[1]) / std[1];
            input_tensor[[0, 2, dest_y, dest_x]] =
                (raw_pixels[idx + 2] as f32 / 255.0 - mean[2]) / std[2];
        }
    }

    let input_tensor_dyn = input_tensor.into_dyn();
    let t_input = Tensor::from_array(input_tensor_dyn.as_standard_layout().into_owned())?;

    let mut session = depth_session.lock().unwrap();
    let outputs = session.run(ort::inputs![t_input])?;
    let output_tensor = outputs[0].try_extract_array::<f32>()?.to_owned();
    let out_slice = output_tensor.as_slice().unwrap();

    let usize_size = DEPTH_INPUT_SIZE as usize;

    let mut min_val = f32::MAX;
    let mut max_val = f32::MIN;
    for y in 0..rh {
        let src_y = y + paste_y;
        for x in 0..rw {
            let src_x = x + paste_x;
            let val = out_slice[src_y * usize_size + src_x];
            min_val = min_val.min(val);
            max_val = max_val.max(val);
        }
    }

    let range = max_val - min_val;
    let scale = if range > 1e-6 { 255.0 / range } else { 0.0 };

    let mut cropped_depth_data = Vec::with_capacity(rw * rh);

    for y in 0..rh {
        let src_y = y + paste_y;
        for x in 0..rw {
            let src_x = x + paste_x;
            let val = out_slice[src_y * usize_size + src_x];
            let pixel = if range > 1e-6 {
                ((val - min_val) * scale) as u8
            } else {
                0
            };
            cropped_depth_data.push(pixel);
        }
    }

    let depth_map = GrayImage::from_raw(resized_w, resized_h, cropped_depth_data)
        .ok_or_else(|| anyhow::anyhow!("Failed to create mask from Depth output"))?;

    Ok(depth_map)
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiSubjectMaskParameters {
    pub start_x: f64,
    pub start_y: f64,
    pub end_x: f64,
    pub end_y: f64,
    #[serde(default)]
    pub mask_data_base64: Option<String>,
    #[serde(default)]
    pub rotation: Option<f32>,
    #[serde(default)]
    pub flip_horizontal: Option<bool>,
    #[serde(default)]
    pub flip_vertical: Option<bool>,
    #[serde(default)]
    pub orientation_steps: Option<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiSkyMaskParameters {
    #[serde(default)]
    pub mask_data_base64: Option<String>,
    #[serde(default)]
    pub rotation: Option<f32>,
    #[serde(default)]
    pub flip_horizontal: Option<bool>,
    #[serde(default)]
    pub flip_vertical: Option<bool>,
    #[serde(default)]
    pub orientation_steps: Option<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiForegroundMaskParameters {
    #[serde(default)]
    pub mask_data_base64: Option<String>,
    #[serde(default)]
    pub rotation: Option<f32>,
    #[serde(default)]
    pub flip_horizontal: Option<bool>,
    #[serde(default)]
    pub flip_vertical: Option<bool>,
    #[serde(default)]
    pub orientation_steps: Option<u8>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AiDepthMaskParameters {
    #[serde(default)]
    pub min_depth: f32,
    #[serde(default)]
    pub max_depth: f32,
    #[serde(default)]
    pub min_fade: f32,
    #[serde(default)]
    pub max_fade: f32,
    #[serde(default)]
    pub feather: f32,
    #[serde(default)]
    pub mask_data_base64: Option<String>,
    #[serde(default)]
    pub rotation: Option<f32>,
    #[serde(default)]
    pub flip_horizontal: Option<bool>,
    #[serde(default)]
    pub flip_vertical: Option<bool>,
    #[serde(default)]
    pub orientation_steps: Option<u8>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn configure_test_ort_dylib_path() {
        if env::var_os("ORT_DYLIB_PATH").is_some() {
            return;
        }

        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let library_name = {
            #[cfg(target_os = "windows")]
            {
                "onnxruntime.dll"
            }
            #[cfg(target_os = "linux")]
            {
                "libonnxruntime.so"
            }
            #[cfg(target_os = "macos")]
            {
                "libonnxruntime.dylib"
            }
            #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
            {
                "libonnxruntime.so"
            }
        };

        for candidate in [
            manifest_dir.join("resources").join(library_name),
            manifest_dir
                .join("target")
                .join("debug")
                .join("resources")
                .join(library_name),
        ] {
            if candidate.exists() {
                unsafe {
                    env::set_var("ORT_DYLIB_PATH", &candidate);
                }
                break;
            }
        }
    }

    #[test]
    fn extract_style_transfer_embedding_smoke_test() -> Result<()> {
        configure_test_ort_dylib_path();
        crate::register_exit_handler();

        let model_dir = get_qraw_models_dir()?;
        let backbone_path = model_dir.join(STYLE_TRANSFER_BACKBONE_FILENAME);
        let preprocess_path = model_dir.join(STYLE_TRANSFER_PREPROCESS_FILENAME);

        if !backbone_path.exists() || !preprocess_path.exists() {
            return Ok(());
        }

        let preprocess_content = fs::read_to_string(preprocess_path)?;
        let preprocess: StyleTransferBackbonePreprocessConfig =
            serde_json::from_str(&preprocess_content)?;

        let _ = ort::init().with_name("AI-StyleTransfer-Test").commit();
        let session = Session::builder()?.commit_from_file(backbone_path)?;
        let backbone_models = StyleTransferBackboneModels {
            session: Mutex::new(session),
            preprocess,
        };

        let image = DynamicImage::ImageRgb8(ImageBuffer::from_fn(320, 240, |x, y| {
            let r = ((x as f32 / 319.0) * 255.0).round() as u8;
            let g = ((y as f32 / 239.0) * 255.0).round() as u8;
            let b = (((x + y) as f32 / 558.0) * 255.0).round() as u8;
            Rgb([r, g, b])
        }));

        let embedding = extract_style_transfer_embedding(&image, &backbone_models)?;
        assert_eq!(embedding.len(), 1536);

        let norm = embedding
            .iter()
            .map(|value| (*value as f64) * (*value as f64))
            .sum::<f64>()
            .sqrt();
        assert!(
            (norm - 1.0).abs() < 1e-3,
            "embedding should be L2-normalized, got {}",
            norm
        );

        std::mem::forget(backbone_models);

        Ok(())
    }
}
