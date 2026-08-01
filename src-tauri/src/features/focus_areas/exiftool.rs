use std::env;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use super::orientation::apply_orientation_to_box;
use super::types::{FocusKind, FocusRegion};

//  ExifTool sidecar — 调用 exiftool 获取对焦坐标
//  所有坐标输出为左上角(L,T)，而非中心(Cx,Cy)
//  Y轴归一化使用 480(等效网格高度), 而非 428(物理传感器行数)
//  AF网格428行映射到图像的480等效单位, 覆盖约89%图像高度
//  优先级: FocusPixel(各品牌像素坐标) > FlexibleSpotPosition > FocalPlaneAFPoint > FocusLocation
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

fn push_number_token(token: &mut String, values: &mut Vec<f32>) {
    if token.chars().any(|c| c.is_ascii_digit()) {
        if let Ok(value) = token.parse::<f32>() {
            values.push(value);
        }
    }
    token.clear();
}

pub(super) fn numbers_from_string(input: &str) -> Vec<f32> {
    let mut values = Vec::new();
    let mut token = String::new();

    for ch in input.chars() {
        if ch.is_ascii_digit() || ch == '-' || ch == '+' || ch == '.' {
            token.push(ch);
        } else {
            push_number_token(&mut token, &mut values);
        }
    }
    push_number_token(&mut token, &mut values);
    values
}

pub(super) fn orientation_from_exiftool_text(orientation: Option<&str>) -> u16 {
    let text = orientation.unwrap_or_default().trim();
    if let Ok(code) = text.parse::<u16>()
        && (1..=8).contains(&code)
    {
        return code;
    }

    let lower = text.to_lowercase();
    let nums = numbers_from_string(text);
    if nums.len() == 1 {
        let code = nums[0].round() as u16;
        if (1..=8).contains(&code) && !lower.contains("rotate") {
            return code;
        }
    }

    let mirror_horizontal =
        lower.contains("mirror horizontal") || lower.contains("mirrored horizontal");
    let mirror_vertical = lower.contains("mirror vertical") || lower.contains("mirrored vertical");
    let rotate_270 = lower.contains("rotate 270");
    let rotate_180 = lower.contains("rotate 180");
    let rotate_90 = lower.contains("rotate 90");

    if mirror_horizontal && rotate_270 {
        5
    } else if mirror_horizontal && rotate_90 {
        7
    } else if rotate_270 {
        8
    } else if rotate_90 {
        6
    } else if rotate_180 {
        3
    } else if mirror_horizontal {
        2
    } else if mirror_vertical {
        4
    } else {
        1
    }
}

fn numbers_from_json_value(value: &serde_json::Value) -> Vec<f32> {
    match value {
        serde_json::Value::Number(n) => n.as_f64().map(|v| vec![v as f32]).unwrap_or_default(),
        serde_json::Value::String(s) => numbers_from_string(s),
        serde_json::Value::Array(items) => items
            .iter()
            .flat_map(numbers_from_json_value)
            .collect::<Vec<_>>(),
        serde_json::Value::Object(map) => map
            .values()
            .flat_map(numbers_from_json_value)
            .collect::<Vec<_>>(),
        serde_json::Value::Bool(v) => vec![if *v { 1.0 } else { 0.0 }],
        serde_json::Value::Null => Vec::new(),
    }
}

fn string_from_json_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        serde_json::Value::Bool(v) => Some(if *v { "1".into() } else { "0".into() }),
        serde_json::Value::Array(items) => {
            let joined = items
                .iter()
                .filter_map(string_from_json_value)
                .collect::<Vec<_>>()
                .join(" ");
            (!joined.is_empty()).then_some(joined)
        }
        serde_json::Value::Object(map) => {
            let joined = map
                .values()
                .filter_map(string_from_json_value)
                .collect::<Vec<_>>()
                .join(" ");
            (!joined.is_empty()).then_some(joined)
        }
        serde_json::Value::Null => None,
    }
}

pub(super) fn normalized_focus_region(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    kind: FocusKind,
    is_primary: bool,
) -> Option<FocusRegion> {
    if !x.is_finite() || !y.is_finite() || !width.is_finite() || !height.is_finite() {
        return None;
    }

    let mut nx = x;
    let mut ny = y;
    let mut nw = width.abs();
    let mut nh = height.abs();

    if nw <= 0.0 || nh <= 0.0 {
        return None;
    }
    if nx < 0.0 {
        nw += nx;
        nx = 0.0;
    }
    if ny < 0.0 {
        nh += ny;
        ny = 0.0;
    }
    if nx >= 1.0 || ny >= 1.0 {
        return None;
    }

    nw = nw.min(1.0 - nx);
    nh = nh.min(1.0 - ny);
    if nw < 0.001 || nh < 0.001 {
        return None;
    }

    Some(FocusRegion {
        x: nx,
        y: ny,
        width: nw,
        height: nh,
        kind,
        is_primary,
    })
}

pub(super) fn focus_kind_from_mode(mode: Option<&str>) -> FocusKind {
    let mode = mode.unwrap_or_default().to_lowercase();
    if mode.contains("eye") {
        FocusKind::Eye
    } else if mode.contains("face") {
        FocusKind::Face
    } else if mode.contains("spot") || mode.contains("single") || mode.contains("flexible") {
        FocusKind::Point
    } else {
        FocusKind::Area
    }
}

const EXIFTOOL_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug)]
struct TimedCommandOutput {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) {
    if !paths.iter().any(|existing| existing == &path) {
        paths.push(path);
    }
}

fn exiftool_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    for var in ["RAPIDRAW_EXIFTOOL", "EXIFTOOL_PATH"] {
        if let Some(value) = env::var_os(var) {
            push_unique_path(&mut candidates, PathBuf::from(value));
        }
    }

    if let Ok(exe_path) = env::current_exe()
        && let Some(exe_dir) = exe_path.parent()
    {
        for name in ["exiftool", "exiftool.exe"] {
            push_unique_path(&mut candidates, exe_dir.join(name));
        }
        if let Some(contents_dir) = exe_dir.parent()
            && contents_dir
                .file_name()
                .is_some_and(|name| name == "Contents")
        {
            for name in ["exiftool", "exiftool.exe"] {
                push_unique_path(&mut candidates, contents_dir.join("Resources").join(name));
            }
        }
    }

    for path in [
        "/opt/homebrew/bin/exiftool",
        "/usr/local/bin/exiftool",
        "/usr/bin/exiftool",
        "C:\\Program Files\\ExifTool\\exiftool.exe",
    ] {
        push_unique_path(&mut candidates, PathBuf::from(path));
    }

    if let Some(paths) = env::var_os("PATH") {
        for dir in env::split_paths(&paths) {
            for name in ["exiftool", "exiftool.exe"] {
                push_unique_path(&mut candidates, dir.join(name));
            }
        }
    }

    candidates
}

fn probe_exiftool(path: &Path) -> bool {
    Command::new(path)
        .arg("-ver")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn resolve_exiftool_path() -> Result<PathBuf, String> {
    static EXIFTOOL_PATH: once_cell::sync::Lazy<Option<PathBuf>> =
        once_cell::sync::Lazy::new(|| {
            exiftool_candidates()
                .into_iter()
                .find(|path| probe_exiftool(path))
        });

    EXIFTOOL_PATH.as_ref().cloned().ok_or_else(|| {
        "未找到 exiftool，请安装 ExifTool 或通过 RAPIDRAW_EXIFTOOL/EXIFTOOL_PATH 指定路径"
            .to_string()
    })
}

fn read_child_pipe<R>(mut pipe: R) -> thread::JoinHandle<Vec<u8>>
where
    R: Read + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = pipe.read_to_end(&mut buffer);
        buffer
    })
}

fn run_command_with_timeout(
    mut command: Command,
    timeout: Duration,
) -> Result<TimedCommandOutput, String> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|e| format!("exiftool 进程启动失败: {}", e))?;
    let stdout_handle = child.stdout.take().map(read_child_pipe);
    let stderr_handle = child.stderr.take().map(read_child_pipe);
    let started_at = Instant::now();

    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = stdout_handle
                    .and_then(|handle| handle.join().ok())
                    .unwrap_or_default();
                let stderr = stderr_handle
                    .and_then(|handle| handle.join().ok())
                    .unwrap_or_default();
                return Ok(TimedCommandOutput {
                    status,
                    stdout,
                    stderr,
                });
            }
            Ok(None) if started_at.elapsed() >= timeout => {
                let _ = child.kill();
                let _ = child.wait();
                if let Some(handle) = stdout_handle {
                    let _ = handle.join();
                }
                if let Some(handle) = stderr_handle {
                    let _ = handle.join();
                }
                return Err(format!("exiftool 超时超过 {} 秒", timeout.as_secs()));
            }
            Ok(None) => thread::sleep(Duration::from_millis(25)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("exiftool 状态检查失败: {}", e));
            }
        }
    }
}

pub(super) fn try_extract_via_exiftool(source_path: &Path) -> Result<Vec<FocusRegion>, String> {
    let exiftool_path = resolve_exiftool_path()?;
    let mut command = Command::new(exiftool_path);
    command
        .arg("-j")
        .arg("-Make")
        .arg("-Model")
        .arg("-ImageWidth")
        .arg("-ImageHeight")
        .arg("-ImageSize")
        .arg("-Orientation")
        .arg("-FocusPixel")
        .arg("-AFAreaXPosition")
        .arg("-AFAreaYPosition")
        .arg("-AFAreaXPositions")
        .arg("-AFAreaYPositions")
        .arg("-AFAreaWidth")
        .arg("-AFAreaHeight")
        .arg("-AFAreaWidths")
        .arg("-AFAreaHeights")
        .arg("-AFImageWidth")
        .arg("-AFImageHeight")
        .arg("-AFPointsInFocus")
        .arg("-AFPointsSelected")
        .arg("-PrimaryAFPoint")
        .arg("-AFDetectionMethod")
        .arg("-FocalPlaneAFPointArea")
        .arg("-FocalPlaneAFPointsUsed")
        .arg("-FocalPlaneAFPointLocation1")
        .arg("-FocalPlaneAFPointLocation2")
        .arg("-FocalPlaneAFPointLocation3")
        .arg("-FocalPlaneAFPointLocation4")
        .arg("-FocalPlaneAFPointLocation5")
        .arg("-FocalPlaneAFPointLocation6")
        .arg("-FocalPlaneAFPointLocation7")
        .arg("-FocalPlaneAFPointLocation8")
        .arg("-FocalPlaneAFPointLocation9")
        .arg("-FocalPlaneAFPointLocation10")
        .arg("-FocalPlaneAFPointLocation11")
        .arg("-FocalPlaneAFPointLocation12")
        .arg("-FocalPlaneAFPointLocation13")
        .arg("-FocalPlaneAFPointLocation14")
        .arg("-FocalPlaneAFPointLocation15")
        .arg("-FlexibleSpotPosition")
        .arg("-FocusLocation")
        .arg("-FocusFrameSize")
        .arg("-AFAreaMode")
        .arg(source_path);

    let output = run_command_with_timeout(command, EXIFTOOL_TIMEOUT)?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("exiftool 退出码非零: {}", stderr.trim()));
    }

    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("exiftool JSON 解析失败: {}", e))?;

    let entry = match json.as_array().and_then(|a| a.first()) {
        Some(e) => e,
        None => return Err("exiftool 返回空结果".into()),
    };

    let string_or =
        |key: &str| -> Option<String> { entry.get(key).and_then(string_from_json_value) };
    let numbers_or = |keys: &[&str]| -> Vec<f32> {
        keys.iter()
            .find_map(|key| {
                let nums = entry
                    .get(*key)
                    .map(numbers_from_json_value)
                    .unwrap_or_default();
                (!nums.is_empty()).then_some(nums)
            })
            .unwrap_or_default()
    };
    let first_number =
        |keys: &[&str]| -> Option<f32> { numbers_or(keys).into_iter().find(|v| v.is_finite()) };
    let dimensions_or =
        |width_keys: &[&str], height_keys: &[&str], size_keys: &[&str]| -> Option<(f32, f32)> {
            if let (Some(w), Some(h)) = (first_number(width_keys), first_number(height_keys))
                && w > 0.0
                && h > 0.0
            {
                return Some((w, h));
            }
            let size = numbers_or(size_keys);
            if size.len() >= 2 && size[0] > 0.0 && size[1] > 0.0 {
                Some((size[0], size[1]))
            } else {
                None
            }
        };

    let af_mode = string_or("AFAreaMode");
    let orientation = string_or("Orientation");
    let orientation_code = orientation_from_exiftool_text(orientation.as_deref());
    let make_lc = string_or("Make").unwrap_or_default().to_lowercase();
    let model_lc = string_or("Model").unwrap_or_default().to_lowercase();
    let is_canon = make_lc.contains("canon");
    let is_nikon = make_lc.contains("nikon");
    let is_powershot = model_lc.contains("powershot");
    log::info!(
        "ExifTool: Make={:?}, Model={:?}, AFAreaMode={:?}, Orientation={:?}({}), path={}",
        string_or("Make"),
        string_or("Model"),
        af_mode,
        orientation,
        orientation_code,
        source_path.display()
    );

    let apply_orientation = |x: f32, y: f32, w: f32, h: f32| -> (f32, f32, f32, f32) {
        apply_orientation_to_box(x, y, w, h, orientation_code)
    };

    // ── 0. FocusPixel (像素坐标, 品牌通用 — Fujifilm等) ──
    // FocusPixel 基准是 EXIF ImageWidth/ImageHeight (非 MakerNotes ImageSize)
    let focus_pixel = numbers_or(&["FocusPixel"]);
    if focus_pixel.len() >= 2 {
        if let Some((iw, ih)) = dimensions_or(&["ImageWidth"], &["ImageHeight"], &["ImageSize"]) {
            let cx = focus_pixel[0] / iw;
            let cy = focus_pixel[1] / ih;
            let marker = 0.02;
            let lx = cx - marker;
            let ly = cy - marker;
            let sz = 0.04;
            if cx >= 0.0 && cx <= 1.0 && cy >= 0.0 && cy <= 1.0 {
                let (nx, ny, nw, nh) = apply_orientation(lx, ly, sz, sz);
                if let Some(region) =
                    normalized_focus_region(nx, ny, nw, nh, FocusKind::Point, true)
                {
                    log::info!(
                        "ExifTool FocusPixel → AF: px=({:.0},{:.0})/{:.0}x{:.0}, display=({:.4},{:.4})",
                        focus_pixel[0],
                        focus_pixel[1],
                        iw,
                        ih,
                        region.x + region.width / 2.0,
                        region.y + region.height / 2.0
                    );
                    return Ok(vec![region]);
                }
            }
        }
    }

    // ── 0b. AFArea (Canon CR2/NEF, 中心原点坐标) ──
    // Canon AFAreaXPositions/YPositions 值为图像中心偏移量(负=左/上), 配合 AFImageWidth/Height
    // Nikon AFAreaXPosition/YPosition 为左上角原点像素坐标
    // AFPointsInFocus 为逗号分隔的索引(如 "35" 或 "0,1,2"), 指示哪些点合焦
    if let Some((iw, ih)) = dimensions_or(&["AFImageWidth"], &["AFImageHeight"], &[]) {
        let xpos = numbers_or(&["AFAreaXPositions"]);
        let ypos = numbers_or(&["AFAreaYPositions"]);
        let xpos_alt = numbers_or(&["AFAreaXPosition"]);
        let ypos_alt = numbers_or(&["AFAreaYPosition"]);
        let xp = if !xpos.is_empty() { xpos } else { xpos_alt };
        let yp = if !ypos.is_empty() { ypos } else { ypos_alt };
        let npts = xp.len().min(yp.len());

        if npts > 0 {
            let widths = numbers_or(&["AFAreaWidths", "AFAreaWidth"]);
            let heights = numbers_or(&["AFAreaHeights", "AFAreaHeight"]);
            let default_w = (iw * 0.04).max(1.0);
            let default_h = (ih * 0.04).max(1.0);
            let value_at = |values: &[f32], index: usize, default_value: f32| -> f32 {
                values
                    .get(index)
                    .copied()
                    .or_else(|| values.first().copied())
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .unwrap_or(default_value)
            };

            let focus_index_numbers = {
                let nums = numbers_or(&["AFPointsInFocus"]);
                if nums.is_empty() {
                    numbers_or(&["AFPointsSelected", "PrimaryAFPoint"])
                } else {
                    nums
                }
            };
            let raw_indices = focus_index_numbers
                .iter()
                .filter_map(|v| {
                    if v.is_finite() && *v >= 0.0 {
                        Some(v.round() as usize)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();
            let mut focus_indices = raw_indices
                .iter()
                .copied()
                .filter(|&i| i < npts)
                .collect::<Vec<_>>();
            if focus_indices.is_empty() && raw_indices.iter().all(|&i| i > 0 && i <= npts) {
                focus_indices = raw_indices.iter().map(|i| i - 1).collect();
            }
            if focus_indices.is_empty() && npts == 1 {
                focus_indices.push(0);
            }

            if !focus_indices.is_empty() {
                let af_detection = string_or("AFDetectionMethod")
                    .unwrap_or_default()
                    .to_lowercase();
                let nikon_center_position = is_nikon && af_detection.contains("contrast");
                let center_origin =
                    is_canon || xp.iter().any(|&v| v < 0.0) || yp.iter().any(|&v| v < 0.0);
                let kind = focus_kind_from_mode(af_mode.as_deref());
                let mut regions = Vec::new();

                for (rank, &fi) in focus_indices.iter().enumerate() {
                    if fi >= npts {
                        continue;
                    }
                    let pw = value_at(&widths, fi, default_w);
                    let ph = value_at(&heights, fi, default_h);
                    let half_w = pw / 2.0;
                    let half_h = ph / 2.0;
                    let x_off = xp[fi];
                    let y_off = yp[fi];

                    let (lx, ly) = if center_origin {
                        let y = if is_canon && !is_powershot {
                            ih / 2.0 - y_off - half_h
                        } else {
                            ih / 2.0 + y_off - half_h
                        };
                        (iw / 2.0 + x_off - half_w, y)
                    } else if nikon_center_position {
                        (x_off - half_w, y_off - half_h)
                    } else {
                        (x_off, y_off)
                    };

                    let (nx, ny, nw, nh) = apply_orientation(lx / iw, ly / ih, pw / iw, ph / ih);
                    if let Some(region) =
                        normalized_focus_region(nx, ny, nw, nh, kind.clone(), rank == 0)
                    {
                        regions.push(region);
                    }
                }

                if !regions.is_empty() {
                    log::info!(
                        "ExifTool AFArea → AF: {} of {} pts, mode={:?}",
                        regions.len(),
                        npts,
                        af_mode
                    );
                    return Ok(regions);
                }
            }
        }
    }

    // ── 1. FlexibleSpotPosition (640×428 网格, 用户对焦点中心 → 输出左上角) ──
    let flexible_spot = numbers_or(&["FlexibleSpotPosition"]);
    if flexible_spot.len() >= 2 && flexible_spot[0] > 0.0 && flexible_spot[1] > 0.0 {
        let cx = flexible_spot[0] / 640.0;
        let cy = flexible_spot[1] / 480.0;
        let marker = 0.015;
        let lx = cx - marker;
        let ly = cy - marker;
        if cx > 0.001 && cx < 0.999 && cy > 0.001 && cy < 0.999 {
            let (nx, ny, nw, nh) = apply_orientation(lx, ly, 0.03, 0.03);
            if let Some(region) = normalized_focus_region(nx, ny, nw, nh, FocusKind::Point, true) {
                log::info!(
                    "ExifTool FlexibleSpotPosition → AF: sensor=({:.4},{:.4}), display=({:.4},{:.4})",
                    cx,
                    cy,
                    region.x + region.width / 2.0,
                    region.y + region.height / 2.0
                );
                return Ok(vec![region]);
            }
        }
    }

    // ── 2. FocalPlaneAFPoint (640×428 网格, AF传感器区域 → 输出包围盒左上角) ──
    // grid_h 来自 ExifTool(FocalPlaneAFPointArea), 为物理428行
    // Y归一化必须用 norm_h=480(等效高度), 而非 grid_h=428
    let focal_plane_area = numbers_or(&["FocalPlaneAFPointArea"]);
    let grid_w: f32 = focal_plane_area.first().copied().unwrap_or(640.0);
    let norm_h: f32 = 480.0;

    let mut af_points: Vec<(f32, f32)> = Vec::new();
    for i in 1..=15 {
        let key = format!("FocalPlaneAFPointLocation{}", i);
        let parts = numbers_or(&[key.as_str()]);
        if parts.len() >= 2 && parts[0] > 0.0 && parts[1] > 0.0 {
            af_points.push((parts[0], parts[1]));
        }
    }

    if !af_points.is_empty() {
        let min_x = af_points.iter().map(|p| p.0).fold(f32::MAX, f32::min);
        let max_x = af_points.iter().map(|p| p.0).fold(0.0_f32, f32::max);
        let min_y = af_points.iter().map(|p| p.1).fold(f32::MAX, f32::min);
        let max_y = af_points.iter().map(|p| p.1).fold(0.0_f32, f32::max);

        let lx = (min_x / grid_w).max(0.0);
        let ly = (min_y / norm_h).max(0.0);
        let lw = ((max_x - min_x + 1.0) / grid_w).max(0.02);
        let lh = ((max_y - min_y + 1.0) / norm_h).max(0.02);
        let (nx, ny, nw, nh) = apply_orientation(lx, ly, lw, lh);

        log::info!(
            "ExifTool FocalPlaneAFPoint → AF: sensor_tl=({:.0},{:.0})_{:.0}x480, {} pts, display=({:.4},{:.4},{:.4},{:.4})",
            min_x,
            min_y,
            grid_w,
            af_points.len(),
            nx,
            ny,
            nw,
            nh
        );

        if let Some(region) = normalized_focus_region(nx, ny, nw, nh, FocusKind::Area, true) {
            return Ok(vec![region]);
        }
    }

    // ── 3. FocusLocation (像素坐标系, 焦点框中心 → 输出左上角) ──
    // FocusLocation 基于 IFD ImageWidth/Height(传感器横拍), 竖拍需旋转
    let focus_location = numbers_or(&["FocusLocation"]);
    if focus_location.len() >= 4 && focus_location[0] > 0.0 && focus_location[1] > 0.0 {
        let img_w = focus_location[0];
        let img_h = focus_location[1];
        let fx = focus_location[2];
        let fy = focus_location[3];

        let cenx = fx / img_w;
        let ceny = fy / img_h;

        let focus_frame_size = numbers_or(&["FocusFrameSize"]);
        let fw = focus_frame_size
            .first()
            .map(|v| (*v / img_w).max(0.01))
            .unwrap_or(0.05);
        let fh = focus_frame_size
            .get(1)
            .map(|v| (*v / img_h).max(0.01))
            .unwrap_or(0.05);

        let lx = cenx - fw / 2.0;
        let ly = ceny - fh / 2.0;
        let (nx, ny, nw, nh) = apply_orientation(lx, ly, fw, fh);

        if let Some(region) = normalized_focus_region(
            nx,
            ny,
            nw,
            nh,
            focus_kind_from_mode(af_mode.as_deref()),
            true,
        ) {
            log::info!(
                "ExifTool FocusLocation → AF: sensor=({:.4},{:.4},{:.4},{:.4}), display=({:.4},{:.4},{:.4},{:.4})",
                cenx,
                ceny,
                fw,
                fh,
                region.x,
                region.y,
                region.width,
                region.height
            );
            return Ok(vec![region]);
        }
    }

    Err("exiftool 未返回可解析的对焦坐标".into())
}
