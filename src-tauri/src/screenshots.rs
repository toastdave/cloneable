use std::path::Path;

use xcap::Monitor;

use crate::error::AppError;
use crate::events::StepScreenshots;

/// Fixed dimensions for the click crop region.
const CLICK_CROP_WIDTH: u32 = 400;
const CLICK_CROP_HEIGHT: u32 = 300;

/// Capture result holding the three screenshot relative paths and fallback flag.
pub struct CaptureResult {
    pub screenshots: StepScreenshots,
}

/// Capture all three screenshots for a single step and save them to `shots_dir`.
///
/// - `step_index`: used to name the files (e.g. `0_full.png`).
/// - `x`, `y`: screen coordinates of the event (used for click crop + monitor selection).
/// - `shots_dir`: absolute path to the `shots/` directory for this session.
///
/// Returns relative paths (relative to the session directory) for each screenshot.
pub fn capture_step_screenshots(
    step_index: usize,
    x: f64,
    y: f64,
    shots_dir: &Path,
) -> Result<CaptureResult, AppError> {
    // --- 1. Full-screen screenshot ---
    let monitor = Monitor::from_point(x as i32, y as i32)
        .map_err(|e| AppError::Screenshot(format!("Monitor::from_point failed: {e}")))?;

    let full_image = monitor
        .capture_image()
        .map_err(|e| AppError::Screenshot(format!("capture_image failed: {e}")))?;

    let full_filename = format!("{step_index}_full.png");
    let full_path = shots_dir.join(&full_filename);
    full_image
        .save(&full_path)
        .map_err(|e| AppError::Screenshot(format!("save full screenshot failed: {e}")))?;

    // --- 2. Click crop (400x300 centered on click, clamped to image bounds) ---
    let click_filename = format!("{step_index}_click.png");
    let click_path = shots_dir.join(&click_filename);
    save_click_crop(&full_image, x, y, &monitor, &click_path)?;

    // --- 3. Window crop (best-effort via active-win-pos-rs) ---
    let (window_filename, window_crop_fallback) =
        capture_window_crop(step_index, &full_image, &monitor, shots_dir)?;

    // Paths stored in JSON are relative to the session directory, e.g. "shots/0_full.png".
    Ok(CaptureResult {
        screenshots: StepScreenshots {
            full_screen: format!("shots/{full_filename}"),
            click_crop: format!("shots/{click_filename}"),
            window_crop: format!("shots/{window_filename}"),
            window_crop_fallback,
        },
    })
}

/// Crop a 400x300 region centered on the click point from the full screenshot.
fn save_click_crop(
    full_image: &image::RgbaImage,
    click_x: f64,
    click_y: f64,
    monitor: &Monitor,
    out_path: &Path,
) -> Result<(), AppError> {
    // Convert click coordinates from screen-space to image-space
    // (subtract monitor origin since the image is just this monitor).
    let monitor_x = monitor.x().unwrap_or(0) as f64;
    let monitor_y = monitor.y().unwrap_or(0) as f64;
    let img_x = (click_x - monitor_x).max(0.0) as u32;
    let img_y = (click_y - monitor_y).max(0.0) as u32;

    let (img_w, img_h) = full_image.dimensions();

    // Compute top-left so the crop is centered on the click, clamped to image bounds.
    let half_w = CLICK_CROP_WIDTH / 2;
    let half_h = CLICK_CROP_HEIGHT / 2;

    let left = if img_x >= half_w {
        (img_x - half_w).min(img_w.saturating_sub(CLICK_CROP_WIDTH))
    } else {
        0
    };
    let top = if img_y >= half_h {
        (img_y - half_h).min(img_h.saturating_sub(CLICK_CROP_HEIGHT))
    } else {
        0
    };

    let crop_w = CLICK_CROP_WIDTH.min(img_w - left);
    let crop_h = CLICK_CROP_HEIGHT.min(img_h - top);

    let cropped = image::imageops::crop_imm(full_image, left, top, crop_w, crop_h).to_image();
    cropped
        .save(out_path)
        .map_err(|e| AppError::Screenshot(format!("save click crop failed: {e}")))?;

    Ok(())
}

/// Attempt to capture a window crop using the active window bounds.
/// Falls back to saving a copy of the full screenshot if window detection fails.
fn capture_window_crop(
    step_index: usize,
    full_image: &image::RgbaImage,
    monitor: &Monitor,
    shots_dir: &Path,
) -> Result<(String, bool), AppError> {
    let filename = format!("{step_index}_window.png");
    let out_path = shots_dir.join(&filename);

    match active_win_pos_rs::get_active_window() {
        Ok(active_window) => {
            let win_pos = active_window.position;
            let win_x = win_pos.x as f64;
            let win_y = win_pos.y as f64;
            let win_w = win_pos.width as u32;
            let win_h = win_pos.height as u32;

            if win_w == 0 || win_h == 0 {
                // Degenerate window size — fall back.
                full_image
                    .save(&out_path)
                    .map_err(|e| AppError::Screenshot(format!("save window fallback failed: {e}")))?;
                return Ok((filename, true));
            }

            // Convert window position from screen-space to image-space.
            let monitor_x = monitor.x().unwrap_or(0) as f64;
            let monitor_y = monitor.y().unwrap_or(0) as f64;
            let img_x = (win_x - monitor_x).max(0.0) as u32;
            let img_y = (win_y - monitor_y).max(0.0) as u32;

            let (img_w, img_h) = full_image.dimensions();
            let crop_x = img_x.min(img_w.saturating_sub(1));
            let crop_y = img_y.min(img_h.saturating_sub(1));
            let crop_w = win_w.min(img_w.saturating_sub(crop_x));
            let crop_h = win_h.min(img_h.saturating_sub(crop_y));

            if crop_w == 0 || crop_h == 0 {
                full_image
                    .save(&out_path)
                    .map_err(|e| AppError::Screenshot(format!("save window fallback failed: {e}")))?;
                return Ok((filename, true));
            }

            let cropped =
                image::imageops::crop_imm(full_image, crop_x, crop_y, crop_w, crop_h).to_image();
            cropped
                .save(&out_path)
                .map_err(|e| AppError::Screenshot(format!("save window crop failed: {e}")))?;

            Ok((filename, false))
        }
        Err(_) => {
            // Window detection failed — save full screenshot as fallback.
            full_image
                .save(&out_path)
                .map_err(|e| AppError::Screenshot(format!("save window fallback failed: {e}")))?;
            Ok((filename, true))
        }
    }
}
