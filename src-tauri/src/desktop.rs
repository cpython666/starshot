use crate::imaging::{self, ImagePayload, Rect};
use crate::recovery::{self, RecoveredSession, RecoveryPoint};
use crate::updater::{self, UpdateStatus, Updates};
use image::{DynamicImage, ImageFormat};
use serde::Serialize;
use std::{
    borrow::Cow,
    collections::HashMap,
    io::Cursor,
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex, MutexGuard,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};
use tauri_plugin_updater::UpdaterExt;

type Result<T> = std::result::Result<T, String>;
const CAPTURE_SHORTCUT: &str = "CommandOrControl+Shift+2";
const PIN_SHORTCUT: &str = "CommandOrControl+Alt+3";

#[derive(Clone, Serialize)]
struct Region {
    #[serde(flatten)]
    rect: Rect,
    label: String,
}
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct FramePayload {
    data_url: String,
    width: u32,
    height: u32,
    regions: Vec<Region>,
    scale_factor: f64,
}
struct Frame {
    image: DynamicImage,
    payload: FramePayload,
}
#[derive(Default)]
struct Context {
    operation: Mutex<()>,
    frames: Mutex<HashMap<String, Frame>>,
    current: Mutex<Option<ImagePayload>>,
    pins: Mutex<HashMap<String, ImagePayload>>,
    sequence: AtomicU64,
    shortcut_error: Mutex<Option<String>>,
}
fn lock<T>(value: &Mutex<T>) -> Result<MutexGuard<'_, T>> {
    value.lock().map_err(|_| "操作状态异常，请重启星镜".into())
}
fn main_only(window: &WebviewWindow) -> Result<()> {
    if window.label() == "main" {
        Ok(())
    } else {
        Err("此操作仅允许在主编辑窗口执行".into())
    }
}
fn show_main(app: &AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.unminimize();
        let _ = main.set_focus();
    }
}
fn report_error(app: &AppHandle, error: &str) {
    show_main(app);
    let _ = app.emit_to("main", "capture-error", error);
}
fn clear_capture(app: &AppHandle) -> Result<()> {
    let context = app.state::<Context>();
    let labels: Vec<String> = lock(&context.frames)?
        .drain()
        .map(|(label, _)| label)
        .collect();
    for label in labels {
        if let Some(window) = app.get_webview_window(&label) {
            let _ = window.destroy();
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn capture_permission(request: bool) -> bool {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGPreflightScreenCaptureAccess() -> bool;
        fn CGRequestScreenCaptureAccess() -> bool;
    }
    unsafe { CGPreflightScreenCaptureAccess() || (request && CGRequestScreenCaptureAccess()) }
}
#[cfg(not(target_os = "macos"))]
fn capture_permission(_: bool) -> bool {
    true
}

struct Prepared {
    label: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    frame: Frame,
}
fn begin_capture(app: &AppHandle) -> Result<()> {
    let context = app.state::<Context>();
    let _operation = context
        .operation
        .try_lock()
        .map_err(|_| "另一项截图操作仍在进行")?;
    app.state::<Updates>().ensure_idle()?;
    if !lock(&context.frames)?.is_empty() {
        return Err("请先完成或取消当前截图".into());
    }
    if !capture_permission(true) {
        return Err(
            "请在系统设置 → 隐私与安全性 → 屏幕与系统音频录制中允许星镜，然后重新打开应用".into(),
        );
    }
    if let Some(main) = app.get_webview_window("main") {
        main.hide().map_err(|e| e.to_string())?;
    }
    // Allow the compositor to remove the editor before freezing the display.
    std::thread::sleep(Duration::from_millis(180));
    let result = (|| -> Result<()> {
        let monitors = xcap::Monitor::all().map_err(|e| format!("无法读取显示器：{e}"))?;
        if monitors.is_empty() {
            return Err("没有发现可截图的显示器".into());
        }
        if monitors.len() > 8 {
            return Err("本版最多同时捕获 8 台显示器".into());
        }
        let windows = xcap::Window::all().unwrap_or_default();
        let own_pid = std::process::id();
        let mut window_bounds = Vec::new();
        for window in windows {
            if window.is_minimized().unwrap_or(true) || window.pid().ok() == Some(own_pid) {
                continue;
            }
            if let (Ok(x), Ok(y), Ok(width), Ok(height)) =
                (window.x(), window.y(), window.width(), window.height())
            {
                // App names suffice for prediction; never retain document/window titles.
                let label = window.app_name().unwrap_or_else(|_| "窗口".into());
                window_bounds.push((x, y, width, height, label));
            }
        }
        let mut prepared = Vec::new();
        let mut total_pixels = 0_u64;
        for (index, monitor) in monitors.into_iter().enumerate() {
            let x = monitor.x().map_err(|e| e.to_string())?;
            let y = monitor.y().map_err(|e| e.to_string())?;
            let width = monitor.width().map_err(|e| e.to_string())?;
            let height = monitor.height().map_err(|e| e.to_string())?;
            let scale = f64::from(monitor.scale_factor().unwrap_or(1.0));
            if !scale.is_finite() || scale <= 0.0 || width == 0 || height == 0 {
                return Err("显示器缩放信息无效".into());
            }
            #[cfg(target_os = "macos")]
            imaging::validate_capture_size(
                (f64::from(width) * scale).round() as u32,
                (f64::from(height) * scale).round() as u32,
            )?;
            #[cfg(not(target_os = "macos"))]
            imaging::validate_capture_size(width, height)?;
            let image = DynamicImage::ImageRgba8(
                monitor
                    .capture_image()
                    .map_err(|e| format!("截图失败，请检查系统录屏权限：{e}"))?,
            );
            imaging::validate_capture_size(image.width(), image.height())?;
            total_pixels += u64::from(image.width()) * u64::from(image.height());
            if total_pixels > 80_000_000 {
                return Err("所有显示器合计超过 8000 万像素，请减少连接的显示器后重试".into());
            }
            // xcap uses macOS points; Windows monitor and window coordinates are physical pixels.
            let coordinate_scale = f64::from(image.width()) / f64::from(width);
            let regions = window_bounds
                .iter()
                .filter_map(|(wx, wy, ww, wh, label)| {
                    imaging::intersect_region(
                        *wx,
                        *wy,
                        *ww,
                        *wh,
                        x,
                        y,
                        coordinate_scale,
                        image.width(),
                        image.height(),
                    )
                    .map(|rect| Region {
                        rect,
                        label: label.clone(),
                    })
                })
                .collect();
            let payload = FramePayload {
                data_url: imaging::png_data_url(&image)?,
                width: image.width(),
                height: image.height(),
                regions,
                scale_factor: scale,
            };
            prepared.push(Prepared {
                label: format!("capture-{index}"),
                x,
                y,
                width,
                height,
                frame: Frame { image, payload },
            });
        }
        for entry in prepared {
            lock(&context.frames)?.insert(entry.label.clone(), entry.frame);
            let window = WebviewWindowBuilder::new(
                app,
                &entry.label,
                WebviewUrl::App("index.html?view=capture".into()),
            )
            .title("星镜 · 选择截图区域")
            .decorations(false)
            .resizable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .shadow(false)
            .visible(false)
            .accept_first_mouse(true)
            .visible_on_all_workspaces(true)
            .build()
            .map_err(|e| format!("无法打开截图选区：{e}"))?;
            #[cfg(target_os = "macos")]
            {
                window
                    .set_size(tauri::LogicalSize::new(
                        f64::from(entry.width),
                        f64::from(entry.height),
                    ))
                    .map_err(|e| e.to_string())?;
                window
                    .set_position(tauri::LogicalPosition::new(
                        f64::from(entry.x),
                        f64::from(entry.y),
                    ))
                    .map_err(|e| e.to_string())?;
            }
            #[cfg(not(target_os = "macos"))]
            {
                // Move before sizing so a mixed-DPI monitor transition cannot rescale the final pixel bounds.
                window
                    .set_position(tauri::PhysicalPosition::new(entry.x, entry.y))
                    .map_err(|e| e.to_string())?;
                window
                    .set_size(tauri::PhysicalSize::new(entry.width, entry.height))
                    .map_err(|e| e.to_string())?;
            }
            window.show().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())?;
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = clear_capture(app);
        show_main(app);
    }
    result
}

#[tauri::command]
async fn start_capture(app: AppHandle, window: WebviewWindow) -> Result<()> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || begin_capture(&app))
        .await
        .map_err(|_| "截图任务异常")?
}
#[tauri::command]
fn get_capture_frame(app: AppHandle, window: WebviewWindow) -> Result<FramePayload> {
    lock(&app.state::<Context>().frames)?
        .get(window.label())
        .map(|f| f.payload.clone())
        .ok_or("截图会话已结束，请重新截图".into())
}
#[tauri::command]
async fn finish_capture(app: AppHandle, window: WebviewWindow, rect: Rect) -> Result<()> {
    let label = window.label().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        let _operation = lock(&context.operation)?;
        app.state::<Updates>().ensure_idle()?;
        let result = {
            let frames = lock(&context.frames)?;
            let frame = frames.get(&label).ok_or("截图会话已结束")?;
            rect.validate(frame.image.width(), frame.image.height())?;
            let image = frame
                .image
                .crop_imm(rect.x, rect.y, rect.width, rect.height);
            let seconds = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            imaging::payload(&image, format!("星镜截图-{seconds}.png"))?
        };
        *lock(&context.current)? = Some(result.clone());
        clear_capture(&app)?;
        show_main(&app);
        app.emit_to("main", "capture-complete", result)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|_| "裁切任务异常")?
}
#[tauri::command]
async fn cancel_capture(app: AppHandle, window: WebviewWindow) -> Result<()> {
    let label = window.label().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        let _operation = lock(&context.operation)?;
        if label != "main" && !lock(&context.frames)?.contains_key(&label) {
            return Err("截图会话已结束".into());
        }
        clear_capture(&app)?;
        show_main(&app);
        Ok(())
    })
    .await
    .map_err(|_| "取消截图失败")?
}
#[tauri::command]
fn get_current_image(app: AppHandle, window: WebviewWindow) -> Result<Option<ImagePayload>> {
    main_only(&window)?;
    Ok(lock(&app.state::<Context>().current)?.clone())
}

#[tauri::command]
async fn save_image(
    app: AppHandle,
    window: WebviewWindow,
    data_url: String,
    suggested_name: String,
) -> Result<Option<String>> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        let _operation = lock(&context.operation)?;
        app.state::<Updates>().ensure_idle()?;
        let image = imaging::decode_data_url(&data_url)?;
        let name = imaging::filename(&suggested_name);
        let file = app
            .dialog()
            .file()
            .set_parent(&window)
            .set_file_name(&name)
            .add_filter("图片 PNG / JPEG", &["png", "jpg", "jpeg"])
            .blocking_save_file();
        let Some(file) = file else {
            return Ok(None);
        };
        let mut path = file.into_path().map_err(|_| "请选择本地文件位置")?;
        let extension = path
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let format = match extension.as_str() {
            "jpg" | "jpeg" => ImageFormat::Jpeg,
            "png" => ImageFormat::Png,
            "" => {
                path.set_extension("png");
                ImageFormat::Png
            }
            _ => return Err("请使用 .png、.jpg 或 .jpeg 文件扩展名".into()),
        };
        let mut encoded = Cursor::new(Vec::new());
        if format == ImageFormat::Jpeg {
            let mut rgba = image.to_rgba8();
            for pixel in rgba.pixels_mut() {
                let alpha = u32::from(pixel[3]);
                for channel in &mut pixel.0[..3] {
                    *channel =
                        ((u32::from(*channel) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
                }
                pixel[3] = 255;
            }
            DynamicImage::ImageRgba8(rgba)
                .to_rgb8()
                .write_to(&mut encoded, format)
                .map_err(|e| format!("图片编码失败：{e}"))?;
        } else {
            image
                .write_to(&mut encoded, format)
                .map_err(|e| format!("图片编码失败：{e}"))?;
        }
        // Same-directory staging makes replacement atomic on macOS and Windows.
        imaging::atomic_save(&path, encoded.get_ref())?;
        Ok(Some(path.to_string_lossy().into_owned()))
    })
    .await
    .map_err(|_| "保存任务异常")?
}
#[tauri::command]
async fn copy_image(app: AppHandle, window: WebviewWindow, data_url: String) -> Result<()> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        let _operation = lock(&context.operation)?;
        app.state::<Updates>().ensure_idle()?;
        let image = imaging::decode_data_url(&data_url)?.into_rgba8();
        arboard::Clipboard::new()
            .map_err(|e| format!("无法打开剪贴板：{e}"))?
            .set_image(arboard::ImageData {
                width: image.width() as usize,
                height: image.height() as usize,
                bytes: Cow::Owned(image.into_raw()),
            })
            .map_err(|e| format!("复制图片失败：{e}"))
    })
    .await
    .map_err(|_| "复制图片任务异常")?
}
fn create_pin(app: &AppHandle, data_url: &str) -> Result<()> {
    let context = app.state::<Context>();
    let _operation = lock(&context.operation)?;
    app.state::<Updates>().ensure_idle()?;
    let image = imaging::decode_data_url(data_url)?;
    let mut pins = lock(&context.pins)?;
    if pins.len() >= 12 {
        return Err("最多同时保留 12 张贴图，请先关闭一些贴图".into());
    }
    let id = context.sequence.fetch_add(1, Ordering::Relaxed);
    let label = format!("pin-{id}");
    let payload = imaging::payload(&image, "星镜贴图".into())?;
    pins.insert(label.clone(), payload);
    drop(pins);
    let scale = (800.0 / f64::from(image.width()))
        .min(650.0 / f64::from(image.height()))
        .min(1.0);
    let result =
        WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html?view=pin".into()))
            .title("星镜 · 桌面贴图")
            .inner_size(
                (f64::from(image.width()) * scale).max(160.0),
                (f64::from(image.height()) * scale + 44.0).max(100.0),
            )
            .min_inner_size(100.0, 80.0)
            .center()
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .skip_taskbar(true)
            .shadow(false)
            .resizable(true)
            .accept_first_mouse(true)
            .visible_on_all_workspaces(true)
            .build();
    if let Err(error) = result {
        lock(&context.pins)?.remove(&label);
        return Err(format!("无法创建贴图：{error}"));
    }
    Ok(())
}
#[tauri::command]
async fn pin_image(app: AppHandle, window: WebviewWindow, data_url: String) -> Result<()> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || create_pin(&app, &data_url))
        .await
        .map_err(|_| "贴图任务异常")?
}
#[tauri::command]
fn get_pin_image(app: AppHandle, window: WebviewWindow) -> Result<ImagePayload> {
    lock(&app.state::<Context>().pins)?
        .get(window.label())
        .cloned()
        .ok_or("贴图不存在".into())
}
#[tauri::command]
fn pin_action(
    app: AppHandle,
    window: WebviewWindow,
    action: String,
    value: Option<f64>,
) -> Result<()> {
    let context = app.state::<Context>();
    let _operation = lock(&context.operation)?;
    app.state::<Updates>().ensure_idle()?;
    let image = lock(&context.pins)?
        .get(window.label())
        .cloned()
        .ok_or("贴图不存在")?;
    match action.as_str() {
        "close" => {
            window.destroy().map_err(|e| e.to_string())?;
            lock(&context.pins)?.remove(window.label());
            Ok(())
        }
        "drag" => window.start_dragging().map_err(|e| e.to_string()),
        "opacity" => {
            let opacity = value
                .filter(|v| v.is_finite() && (0.15..=1.0).contains(v))
                .ok_or("透明度应为 15%–100%")?;
            window
                .emit("pin-opacity", opacity)
                .map_err(|e| e.to_string())
        }
        "scale" => {
            let scale = value
                .filter(|v| v.is_finite() && (0.1..=4.0).contains(v))
                .ok_or("贴图缩放应为 10%–400%")?;
            let base = (800.0 / f64::from(image.width))
                .min(650.0 / f64::from(image.height))
                .min(1.0);
            window
                .set_size(tauri::LogicalSize::new(
                    (f64::from(image.width) * base * scale).clamp(100.0, 8000.0),
                    (f64::from(image.height) * base * scale + 44.0).clamp(80.0, 8000.0),
                ))
                .map_err(|e| e.to_string())
        }
        "ignore-cursor" => window
            .set_ignore_cursor_events(value == Some(1.0))
            .map_err(|e| e.to_string()),
        _ => Err("未知贴图操作".into()),
    }
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AppStatus {
    platform: String,
    capture_available: bool,
    shortcut: String,
    error: Option<String>,
}
#[tauri::command]
fn get_app_status(app: AppHandle, window: WebviewWindow) -> Result<AppStatus> {
    main_only(&window)?;
    let permission = capture_permission(false);
    let shortcut_error = lock(&app.state::<Context>().shortcut_error)?.clone();
    Ok(AppStatus {
        platform: std::env::consts::OS.into(),
        capture_available: permission,
        shortcut: CAPTURE_SHORTCUT.into(),
        error: shortcut_error.or_else(|| {
            if permission {
                None
            } else {
                Some("首次截图时需要授予系统屏幕录制权限".into())
            }
        }),
    })
}
fn dispatch_capture(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(error) = begin_capture(&app) {
            let context = app.state::<Context>();
            let capture_in_progress = context.operation.try_lock().is_err()
                || context.frames.lock().is_ok_and(|frames| !frames.is_empty());
            if capture_in_progress {
                // Repeated shortcuts must not focus the editor over an active selection.
                let _ = app.emit_to("main", "capture-error", &error);
            } else {
                report_error(&app, &error);
            }
        }
    });
}
fn dispatch_pin(app: &AppHandle) {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let result = (|| -> Result<()> {
            let current = lock(&app.state::<Context>().current)?
                .clone()
                .ok_or("还没有截图，请先截图或在编辑器中点击贴图")?;
            create_pin(&app, &current.data_url)
        })();
        if let Err(error) = result {
            report_error(&app, &error);
        }
    });
}

fn no_active_capture(context: &Context) -> Result<()> {
    if lock(&context.frames)?.is_empty() {
        Ok(())
    } else {
        Err("请先完成或取消当前截图，再更新或保存恢复点".into())
    }
}
#[tauri::command]
fn get_update_status(app: AppHandle, window: WebviewWindow) -> Result<UpdateStatus> {
    main_only(&window)?;
    app.state::<Updates>().status()
}
#[tauri::command]
async fn check_starshot_update(app: AppHandle, window: WebviewWindow) -> Result<UpdateStatus> {
    main_only(&window)?;
    {
        let context = app.state::<Context>();
        let _operation = context
            .operation
            .try_lock()
            .map_err(|_| "另一项编辑操作仍在进行")?;
        no_active_capture(&context)?;
        app.state::<Updates>().begin_check(&app)?;
    }
    let before_exit = app.clone();
    let cleanup_started = app.state::<Updates>().cleanup_flag()?;
    let result = async {
        let update = app
            .updater_builder()
            .timeout(Duration::from_secs(30))
            .on_before_exit(move || {
                // Windows invokes this immediately before launching its installer. A later
                // launch failure must restart the preserved app without reusing this session.
                cleanup_started.store(true, Ordering::SeqCst);
                // The installer exits directly, bypassing RunEvent::Exit. Release the
                // instance lock before cleanup so a failed installer can reopen safely.
                tauri_plugin_single_instance::destroy(&before_exit);
                before_exit.cleanup_before_exit();
            })
            .build()
            .map_err(|error| format!("更新配置无法读取：{error}"))?
            .check()
            .await
            .map_err(|error| {
                format!("暂时无法检查更新。发布源可能尚未上线或网络不可用，请稍后重试。{error}")
            })?;
        app.state::<Updates>().checked(&app, update)
    }
    .await;
    if let Err(error) = result {
        return app.state::<Updates>().failed(&app, error);
    }
    app.state::<Updates>().status()
}
#[tauri::command]
async fn install_starshot_update(
    app: AppHandle,
    window: WebviewWindow,
    expected_version: String,
    session: String,
) -> Result<UpdateStatus> {
    main_only(&window)?;
    // Fail without starting network activity when the supplied session cannot be preserved.
    let session = tauri::async_runtime::spawn_blocking(move || {
        recovery::validate_session(&session)?;
        Ok::<_, String>(session)
    })
    .await
    .map_err(|_| "检查编辑会话失败")??;
    let (update, cleanup_started) = {
        let context = app.state::<Context>();
        let _operation = context
            .operation
            .try_lock()
            .map_err(|_| "另一项编辑操作仍在进行")?;
        no_active_capture(&context)?;
        app.state::<Updates>()
            .begin_install(&app, &expected_version)?
    };
    // Cache process-only restart data before cleanup invalidates Tauri resources.
    let restart_env = app.env();
    let result: Result<()> = async {
        let mut downloaded = 0_u64;
        let mut last_progress = Instant::now();
        // The official plugin verifies the signature (including signed version) here.
        let bytes = update
            .download(
                |chunk, total| {
                    downloaded += chunk as u64;
                    if last_progress.elapsed() >= Duration::from_millis(100)
                        || total == Some(downloaded)
                    {
                        app.state::<Updates>().progress(&app, downloaded, total);
                        last_progress = Instant::now();
                    }
                },
                || {},
            )
            .await
            .map_err(|error| format!("下载或签名校验失败，未开始安装：{error}"))?;
        app.state::<Updates>()
            .progress(&app, downloaded, Some(downloaded));
        app.state::<Updates>().phase(&app, "backing-up")?;
        let worker_app = app.clone();
        let worker_cleanup_started = cleanup_started.clone();
        let worker_restart_env = restart_env.clone();
        tauri::async_runtime::spawn_blocking(move || -> Result<()> {
            let directory = worker_app
                .path()
                .app_data_dir()
                .map_err(|error| error.to_string())?;
            let point = recovery::create(&directory, &session, "before-update")?;
            worker_app
                .state::<Updates>()
                .backed_up(&worker_app, point.id.clone())?;
            updater::preserve_application(&directory, &point.id)?;
            worker_app
                .state::<Updates>()
                .phase(&worker_app, "installing")?;
            let install_result = update.install(bytes);
            // Windows success exits inside the updater. A returned failure after its
            // cleanup hook cannot safely emit events or reuse hidden webviews.
            if worker_cleanup_started.load(Ordering::SeqCst) {
                tauri::process::restart(&worker_restart_env);
            }
            install_result
                .map_err(|error| format!("安装未完成，旧版应用和编辑会话恢复点已保留：{error}"))?;
            worker_app
                .state::<Updates>()
                .phase(&worker_app, "installed")?;
            Ok(())
        })
        .await
        .map_err(|_| "更新任务异常终止；已生成的恢复点仍保留".to_string())?
    }
    .await;
    if cleanup_started.load(Ordering::SeqCst) {
        // Also covers a worker panic after cleanup: do not touch AppHandle again.
        tauri::process::restart(&restart_env);
    }
    if let Err(error) = result {
        return app.state::<Updates>().failed(&app, error);
    }
    app.state::<Updates>().status()
}
#[tauri::command]
async fn list_recovery_points(app: AppHandle, window: WebviewWindow) -> Result<Vec<RecoveryPoint>> {
    main_only(&window)?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || recovery::list(&directory))
        .await
        .map_err(|_| "读取恢复点列表失败")?
}
#[tauri::command]
async fn create_recovery_point(
    app: AppHandle,
    window: WebviewWindow,
    session: String,
) -> Result<RecoveryPoint> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        let _operation = lock(&context.operation)?;
        app.state::<Updates>().ensure_idle()?;
        no_active_capture(&context)?;
        let directory = app
            .path()
            .app_data_dir()
            .map_err(|error| error.to_string())?;
        recovery::create(&directory, &session, "manual")
    })
    .await
    .map_err(|_| "创建恢复点失败")?
}
#[tauri::command]
async fn read_recovery_point(
    app: AppHandle,
    window: WebviewWindow,
    id: String,
) -> Result<RecoveredSession> {
    main_only(&window)?;
    app.state::<Updates>().ensure_idle()?;
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || recovery::read(&directory, &id))
        .await
        .map_err(|_| "读取恢复点失败")?
}
#[tauri::command]
fn restart_app(app: AppHandle, window: WebviewWindow) -> Result<()> {
    main_only(&window)?;
    app.state::<Updates>().allow_restart()?;
    app.restart();
}

pub fn run() {
    tauri::Builder::default()
        .manage(Context::default())
        .manage(Updates::new())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_main(app)
        }))
        .plugin(tauri_plugin_dialog::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if event.state() != ShortcutState::Pressed {
                        return;
                    }
                    if CAPTURE_SHORTCUT.parse::<Shortcut>().ok().as_ref() == Some(shortcut) {
                        dispatch_capture(app);
                    } else if PIN_SHORTCUT.parse::<Shortcut>().ok().as_ref() == Some(shortcut) {
                        dispatch_pin(app);
                    }
                })
                .build(),
        )
        .setup(|app| {
            let mut errors = Vec::new();
            for text in [CAPTURE_SHORTCUT, PIN_SHORTCUT] {
                if let Err(error) = text
                    .parse::<Shortcut>()
                    .map_err(|e| e.to_string())
                    .and_then(|shortcut| {
                        app.global_shortcut()
                            .register(shortcut)
                            .map_err(|e| e.to_string())
                    })
                {
                    errors.push(format!("快捷键 {text} 注册失败：{error}"));
                }
            }
            if !errors.is_empty() {
                *lock(&app.state::<Context>().shortcut_error)? = Some(errors.join("；"));
            }
            let show = MenuItem::with_id(app, "show", "打开星镜编辑器", true, None::<&str>)?;
            let capture = MenuItem::with_id(app, "capture", "开始截图", true, None::<&str>)?;
            let pin = MenuItem::with_id(app, "pin", "贴出最近截图原图", true, None::<&str>)?;
            let recover =
                MenuItem::with_id(app, "recover", "恢复所有贴图鼠标操作", true, None::<&str>)?;
            let close = MenuItem::with_id(app, "close-pins", "关闭全部贴图", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "退出星镜", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &capture, &pin, &recover, &close, &quit])?;
            // Tray glyphs need a transparent, readable silhouette at menu-bar sizes.
            // macOS recolors its alpha mask; other platforms use a light/dark outline.
            let tray_bytes: &[u8] = if cfg!(target_os = "macos") {
                include_bytes!("../icons/tray-template/36x36.png")
            } else {
                include_bytes!("../icons/tray-contrast/32x32.png")
            };
            let pixels = image::load_from_memory(tray_bytes)?.into_rgba8();
            let (width, height) = pixels.dimensions();
            let tray = TrayIconBuilder::with_id("starshot-tray")
                .icon(tauri::image::Image::new_owned(
                    pixels.into_raw(),
                    width,
                    height,
                ))
                .icon_as_template(cfg!(target_os = "macos"))
                .tooltip("星镜 Starshot")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main(app),
                    "capture" => dispatch_capture(app),
                    "pin" => dispatch_pin(app),
                    "recover" => {
                        if let Err(error) = app.state::<Updates>().ensure_idle() {
                            report_error(app, &error);
                            return;
                        }
                        for (label, window) in app.webview_windows() {
                            if label.starts_with("pin-") {
                                let _ = window.set_ignore_cursor_events(false);
                                let _ = window.show();
                            }
                        }
                    }
                    "close-pins" => {
                        if let Err(error) = app.state::<Updates>().ensure_idle() {
                            report_error(app, &error);
                            return;
                        }
                        for (label, window) in app.webview_windows() {
                            if label.starts_with("pin-") {
                                let _ = window.destroy();
                            }
                        }
                        if let Ok(mut pins) = app.state::<Context>().pins.lock() {
                            pins.clear();
                        }
                    }
                    "quit" => {
                        if app.state::<Updates>().installing() {
                            report_error(app, "正在保存恢复点或安装更新，请等待完成后退出");
                        } else {
                            app.exit(0);
                        }
                    }
                    _ => {}
                });
            tray.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                } else if window.label().starts_with("capture-") {
                    api.prevent_close();
                    let app = window.app_handle().clone();
                    tauri::async_runtime::spawn_blocking(move || {
                        let context = app.state::<Context>();
                        if let Ok(_operation) = context.operation.lock() {
                            let _ = clear_capture(&app);
                            show_main(&app);
                        };
                    });
                }
            }
            if let tauri::WindowEvent::Destroyed = event {
                if window.label().starts_with("pin-") {
                    if let Ok(mut pins) = window.app_handle().state::<Context>().pins.lock() {
                        pins.remove(window.label());
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            start_capture,
            get_capture_frame,
            finish_capture,
            cancel_capture,
            get_current_image,
            save_image,
            copy_image,
            pin_image,
            get_pin_image,
            pin_action,
            get_app_status,
            get_update_status,
            check_starshot_update,
            install_starshot_update,
            list_recovery_points,
            create_recovery_point,
            read_recovery_point,
            restart_app
        ])
        .build(tauri::generate_context!())
        .expect("星镜无法启动")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                if app.state::<Updates>().installing() {
                    api.prevent_exit();
                }
            }
        });
}
