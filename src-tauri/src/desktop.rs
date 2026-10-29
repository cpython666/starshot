use crate::desktop_settings::{self, DesktopSettings, DesktopSettingsStatus};
use crate::imaging::{self, ImagePayload, Rect};
use crate::recovery::{self, RecoveredSession, RecoveryPoint, RecoveryStorage};
use crate::scrolling::ScrollImage;
use crate::updater::{self, UpdateStatus, Updates};
use image::{DynamicImage, ImageFormat};
use serde::{Deserialize, Serialize};
use std::{
    borrow::Cow,
    collections::HashMap,
    io::Cursor,
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc, Arc, Mutex, MutexGuard,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
};
use tauri_plugin_autostart::ManagerExt as AutostartExt;
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use tauri_plugin_updater::UpdaterExt;

type Result<T> = std::result::Result<T, String>;

#[cfg(all(feature = "windows-uia-smoke", target_os = "windows"))]
pub(crate) mod windows_integration;
#[cfg(all(feature = "windows-uia-smoke", target_os = "windows"))]
pub(crate) mod windows_smoke;

// Unknown frontend state is intentionally unsafe to discard, including during WebView startup.
#[derive(Default)]
struct EditorLifecycle {
    reported: bool,
    has_session: bool,
    dirty: bool,
    busy: bool,
    exit_pending: bool,
    exit_approved: bool,
    exit_announced: bool,
    release_request: Option<u64>,
    released: bool,
    release_startup_when_ready: bool,
}
impl EditorLifecycle {
    fn empty_idle(&self) -> bool {
        self.reported && !self.has_session && !self.dirty && !self.busy && !self.exit_pending
    }
    fn update(&mut self, has_session: bool, dirty: bool, busy: bool) {
        self.reported = true;
        self.has_session = has_session;
        self.dirty = dirty;
        self.busy = busy;
        if has_session || dirty || busy {
            self.release_request = None;
        }
    }
    fn confirm_exit(&mut self) -> Result<()> {
        if !self.exit_pending || !self.reported || self.busy {
            return Err("编辑状态尚未就绪或正在处理，请稍后重试退出".into());
        }
        self.exit_approved = true;
        Ok(())
    }
    fn release(&mut self, request: u64) -> Result<()> {
        if !self.empty_idle() || self.release_request != Some(request) {
            return Err("编辑状态已变化，保留编辑器".into());
        }
        self.release_request = None;
        self.released = true;
        self.reported = false;
        Ok(())
    }
}

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
    #[serde(skip_serializing_if = "Option::is_none")]
    region_warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pointer: Option<crate::window_regions::ImagePoint>,
    element_detection: bool,
}
#[derive(Clone, Copy, Debug, PartialEq)]
struct CaptureMonitor {
    id: u32,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    scale: f64,
    image_width: u32,
    image_height: u32,
}
struct Frame {
    presented: bool,
    image: DynamicImage,
    payload: FramePayload,
    monitor: CaptureMonitor,
    hosts: Vec<CaptureHost>,
    element_generation: Arc<AtomicU64>,
    element_point: Option<crate::window_regions::ImagePoint>,
}
#[derive(Clone, Copy, PartialEq)]
struct CaptureHost {
    pid: u32,
    rect: Rect,
    window: crate::window_regions::WindowBounds,
}
struct ScrollCapture {
    label: String,
    monitor: CaptureMonitor,
    rect: Rect,
    stitch: ScrollImage,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScrollProgress {
    width: u32,
    height: u32,
    frames: u32,
    preview_data_url: String,
    message: String,
}
impl ScrollCapture {
    fn progress(&self, message: &str) -> Result<ScrollProgress> {
        let image = self.stitch.image();
        let preview = image::imageops::thumbnail(image, 240, 600);
        Ok(ScrollProgress {
            width: image.width(),
            height: image.height(),
            frames: self.stitch.frames(),
            preview_data_url: imaging::png_data_url(&DynamicImage::ImageRgba8(preview))?,
            message: message.into(),
        })
    }
}
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
enum CaptureAction {
    #[default]
    Edit,
    Copy,
    Save,
    Pin,
}
#[derive(Serialize)]
struct CaptureOutcome {
    completed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
}
#[derive(Default)]
struct Context {
    operation: Mutex<()>,
    frames: Mutex<HashMap<String, Frame>>,
    scroll: Mutex<Option<ScrollCapture>>,
    current: Mutex<Option<ImagePayload>>,
    pending_pin_edit: Mutex<Option<PendingPinEdit>>,
    pins: Mutex<HashMap<String, ImagePayload>>,
    sequence: AtomicU64,
    shortcut_error: Mutex<Option<String>>,
    settings: Mutex<DesktopSettings>,
    editor: Mutex<EditorLifecycle>,
    last_saved: Mutex<Option<std::path::PathBuf>>,
}
impl Context {
    fn ensure_not_exiting(&self) -> Result<()> {
        let mut editor = lock(&self.editor)?;
        if editor.exit_pending {
            Err("请先完成或取消退出确认".into())
        } else {
            editor.release_request = None;
            Ok(())
        }
    }
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
// Must run on the UI thread. Callers which promise delivery need the real
// window-creation result instead of treating a queued show request as success.
fn show_main_window(app: &AppHandle) -> Result<()> {
    let context = app.state::<Context>();
    let main = if let Some(main) = app.get_webview_window("main") {
        main
    } else {
        let config = app
            .config()
            .app
            .windows
            .iter()
            .find(|config| config.label == "main")
            .ok_or("找不到编辑器窗口配置")?;
        let mut editor = lock(&context.editor)?;
        editor.released = false;
        editor.reported = false;
        editor.release_request = None;
        drop(editor);
        WebviewWindowBuilder::from_config(app, config)
            .map_err(|error| error.to_string())?
            .build()
            .map_err(|error| error.to_string())?
    };
    let mut editor = lock(&context.editor)?;
    editor.release_request = None;
    editor.release_startup_when_ready = false;
    drop(editor);
    main.show().map_err(|error| error.to_string())?;
    main.unminimize().map_err(|error| error.to_string())?;
    main.set_focus().map_err(|error| error.to_string())
}
fn show_main(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let _ = show_main_window(&handle);
    });
}
fn report_error(app: &AppHandle, error: &str) {
    show_main(app);
    let _ = app.emit_to("main", "capture-error", error);
}
fn clear_capture(app: &AppHandle) -> Result<()> {
    let context = app.state::<Context>();
    lock(&context.scroll)?.take();
    let labels: Vec<String> = lock(&context.frames)?
        .drain()
        .map(|(label, frame)| {
            frame.element_generation.store(u64::MAX, Ordering::Release);
            label
        })
        .collect();
    for label in labels {
        if let Some(window) = app.get_webview_window(&label) {
            let _ = window.destroy();
        }
    }
    Ok(())
}

const CAPTURE_PERMISSION_HELP: &str =
    "请在系统设置 → 隐私与安全性 → 屏幕与系统音频录制中允许 Starshot（星镜），然后重新打开应用。";

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
    context.ensure_not_exiting()?;
    app.state::<Updates>().ensure_idle()?;
    if !lock(&context.frames)?.is_empty() {
        return Err("请先完成或取消当前截图".into());
    }
    if !capture_permission(true) {
        return Err(CAPTURE_PERMISSION_HELP.into());
    }
    let main_was_visible = if let Some(main) = app.get_webview_window("main") {
        let visible = main.is_visible().map_err(|e| e.to_string())?;
        if visible {
            main.hide().map_err(|e| e.to_string())?;
        }
        visible
    } else {
        false
    };
    if main_was_visible {
        // Only wait when the compositor actually has an editor to remove.
        std::thread::sleep(Duration::from_millis(180));
    }
    let result = (|| -> Result<()> {
        let monitors = xcap::Monitor::all().map_err(|e| format!("无法读取显示器：{e}"))?;
        if monitors.is_empty() {
            return Err("没有发现可截图的显示器".into());
        }
        if monitors.len() > 8 {
            return Err("本版最多同时捕获 8 台显示器".into());
        }
        let (window_bounds, region_warning) =
            match crate::window_regions::snapshot(std::process::id()) {
                Ok(bounds) if !bounds.is_empty() => (bounds, None),
                Ok(bounds) => (
                    bounds,
                    Some("未找到可用的窗口边界，可继续按画面边缘预选或手动框选。".into()),
                ),
                Err(_) => (
                    Vec::new(),
                    Some("暂时无法读取窗口边界，可继续按画面边缘预选或手动框选。".into()),
                ),
            };
        let mut prepared = Vec::new();
        let mut total_pixels = 0_u64;
        // Old queued finish/cancel requests must never address a later capture.
        let capture_id = context.sequence.fetch_add(1, Ordering::Relaxed);
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
            let image = capture_pixels(app, &monitor, width, height)?;
            imaging::validate_capture_size(image.width(), image.height())?;
            total_pixels += u64::from(image.width()) * u64::from(image.height());
            if total_pixels > 80_000_000 {
                return Err("所有显示器合计超过 8000 万像素，请减少连接的显示器后重试".into());
            }
            // xcap uses macOS points; Windows monitor and window coordinates are physical pixels.
            let coordinate_scale = f64::from(image.width()) / f64::from(width);
            let hosts: Vec<_> = window_bounds
                .iter()
                .filter_map(|bounds| {
                    imaging::intersect_region(
                        bounds.x,
                        bounds.y,
                        bounds.width,
                        bounds.height,
                        x,
                        y,
                        coordinate_scale,
                        image.width(),
                        image.height(),
                    )
                    .map(|rect| CaptureHost {
                        pid: bounds.pid,
                        rect,
                        window: *bounds,
                    })
                })
                .collect();
            let payload = FramePayload {
                data_url: imaging::png_data_url(&image)?,
                width: image.width(),
                height: image.height(),
                regions: hosts
                    .iter()
                    .map(|host| Region {
                        rect: host.rect,
                        label: "窗口".into(),
                    })
                    .collect(),
                scale_factor: scale,
                region_warning: region_warning.clone(),
                pointer: None,
                element_detection: crate::window_regions::element_detection_available(),
            };
            prepared.push(Prepared {
                label: format!("capture-{capture_id}-{index}"),
                x,
                y,
                width,
                height,
                frame: Frame {
                    presented: false,
                    monitor: CaptureMonitor {
                        id: monitor.id().map_err(|e| e.to_string())?,
                        x,
                        y,
                        width,
                        height,
                        scale,
                        image_width: image.width(),
                        image_height: image.height(),
                    },
                    image,
                    payload,
                    hosts,
                    element_generation: Arc::new(AtomicU64::new(0)),
                    element_point: None,
                },
            });
        }
        let labels = prepared.iter().map(|entry| entry.label.clone()).collect();
        for entry in prepared {
            lock(&context.frames)?.insert(entry.label.clone(), entry.frame);
            let builder = WebviewWindowBuilder::new(
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
            .visible_on_all_workspaces(true);
            #[cfg(all(feature = "windows-uia-smoke", target_os = "windows"))]
            let builder = windows_integration::configure(app, builder);
            let window = builder
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
        }
        watch_capture_startup(app.clone(), labels);
        Ok(())
    })();
    if result.is_err() {
        let _ = clear_capture(app);
        show_main(app);
    }
    result
}

fn watch_capture_startup(app: AppHandle, labels: Vec<String>) {
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        let pending = || {
            context.frames.lock().is_ok_and(|frames| {
                labels
                    .iter()
                    .any(|label| frames.get(label).is_some_and(|frame| !frame.presented))
            })
        };
        let started = Instant::now();
        while pending() {
            if started.elapsed() >= Duration::from_secs(10) {
                // Recheck after acquiring the operation lock: an old timeout must
                // not close a later capture, or race a successful ready message.
                if let Ok(_operation) = context.operation.lock() {
                    if pending() {
                        let _ = clear_capture(&app);
                        report_error(&app, "截图界面加载超时，请重新截图。");
                    }
                }
                return;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    });
}

#[tauri::command]
async fn capture_overlay_ready(
    app: AppHandle,
    window: WebviewWindow,
    error: Option<String>,
) -> Result<()> {
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        // The builder must finish positioning all hidden windows before any show.
        let _operation = lock(&context.operation)?;
        context.ensure_not_exiting()?;
        if lock(&context.scroll)?.is_some() {
            return Err("长截图已开始".into());
        }
        let presented = lock(&context.frames)?
            .get(window.label())
            .ok_or("截图会话已结束")?
            .presented;
        if let Some(error) = error {
            clear_capture(&app)?;
            report_error(
                &app,
                &format!(
                    "截图界面加载失败：{}",
                    error.chars().take(500).collect::<String>()
                ),
            );
        } else if !presented {
            // Vue has decoded the actual displayed image and committed the DOM.
            window.show().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())?;
            if let Some(frame) = lock(&context.frames)?.get_mut(window.label()) {
                frame.presented = true;
            }
        }
        Ok(())
    })
    .await
    .map_err(|_| "显示截图界面失败")?
}

fn capture_pixels(
    _app: &AppHandle,
    monitor: &xcap::Monitor,
    _width: u32,
    _height: u32,
) -> Result<DynamicImage> {
    #[cfg(all(feature = "windows-uia-smoke", target_os = "windows"))]
    if let Some(image) = windows_integration::pixels(_app, _width, _height) {
        return Ok(image);
    }
    Ok(DynamicImage::ImageRgba8(monitor.capture_image().map_err(
        |e| format!("截图失败，请检查系统录屏权限：{e}"),
    )?))
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
    if lock(&app.state::<Context>().scroll)?.is_some() {
        return Err("长截图已开始，请使用长截图操作栏".into());
    }
    let (mut payload, _monitor) = lock(&app.state::<Context>().frames)?
        .get(window.label())
        .map(|f| (f.payload.clone(), f.monitor))
        .ok_or("截图会话已结束，请重新截图")?;
    payload.pointer = (|| {
        let cursor = window.cursor_position().ok()?;
        #[cfg(target_os = "windows")]
        let (origin, viewport) = (
            tauri::PhysicalPosition::new(_monitor.x, _monitor.y),
            tauri::PhysicalSize::new(_monitor.width, _monitor.height),
        );
        #[cfg(not(target_os = "windows"))]
        let (origin, viewport) = (window.inner_position().ok()?, window.inner_size().ok()?);
        #[cfg(target_os = "macos")]
        let (cursor_scale, window_scale) = (
            window.primary_monitor().ok()??.scale_factor(),
            window.scale_factor().ok()?,
        );
        #[cfg(not(target_os = "macos"))]
        let (cursor_scale, window_scale) = (1.0, 1.0);
        crate::window_regions::cursor_in_image(
            (cursor.x, cursor.y),
            cursor_scale,
            (f64::from(origin.x), f64::from(origin.y)),
            window_scale,
            (viewport.width, viewport.height),
            (payload.width, payload.height),
        )
    })();
    Ok(payload)
}

#[tauri::command]
fn invalidate_capture_element(
    app: AppHandle,
    window: WebviewWindow,
    generation: u64,
) -> Result<()> {
    let context = app.state::<Context>();
    let mut frames = lock(&context.frames)?;
    let Some(frame) = frames.get_mut(window.label()) else {
        return Ok(());
    };
    // A delayed invalidation for this same request must not erase a newer point.
    if generation > frame.element_generation.load(Ordering::Acquire) {
        frame
            .element_generation
            .store(generation, Ordering::Release);
        frame.element_point = None;
    }
    Ok(())
}

#[tauri::command]
async fn get_capture_element(
    app: AppHandle,
    window: WebviewWindow,
    point: crate::window_regions::ImagePoint,
    generation: u64,
) -> Result<Vec<Region>> {
    let label = window.label().to_string();
    // Snapshot synchronously BEFORE spawn_blocking: stale queued work cannot
    // acquire a new generation later. No COM call occurs under either mutex.
    let snapshot = {
        let context = app.state::<Context>();
        let Ok(_operation) = context.operation.try_lock() else {
            #[cfg(all(feature = "windows-uia-smoke", target_os = "windows"))]
            crate::window_regions::windows::trace_gate(&label, generation, "operation-busy");
            return Ok(Vec::new());
        };
        context.ensure_not_exiting()?;
        if lock(&context.scroll)?.is_some() {
            return Ok(Vec::new());
        }
        let mut frames = lock(&context.frames)?;
        let frame = frames.get_mut(&label).ok_or("截图会话已结束")?;
        if point.x >= frame.payload.width || point.y >= frame.payload.height {
            return Err("鼠标位置超出当前截图".into());
        }
        if !frame.payload.element_detection
            || generation < frame.element_generation.load(Ordering::Acquire)
        {
            return Ok(Vec::new());
        }
        frame
            .element_generation
            .store(generation, Ordering::Release);
        frame.element_point = Some(point);
        let Some(host) = frame.hosts.iter().find(|host| {
            point.x >= host.rect.x
                && point.y >= host.rect.y
                && point.x < host.rect.x + host.rect.width
                && point.y < host.rect.y + host.rect.height
        }) else {
            return Ok(Vec::new());
        };
        (*host, frame.monitor, frame.element_generation.clone())
    };
    tauri::async_runtime::spawn_blocking(move || {
        let (host, monitor, revision) = snapshot;
        let position = (
            f64::from(monitor.x)
                + f64::from(point.x) * f64::from(monitor.width) / f64::from(monitor.image_width),
            f64::from(monitor.y)
                + f64::from(point.y) * f64::from(monitor.height) / f64::from(monitor.image_height),
        );
        #[cfg(target_os = "windows")]
        let bounds = crate::window_regions::windows::query(
            crate::window_regions::windows::Request {
                target: host.window,
                x: position.0.floor() as i32,
                y: position.1.floor() as i32,
            },
            &revision,
            generation,
        );
        #[cfg(not(target_os = "windows"))]
        let bounds: Vec<_> = crate::window_regions::element_at(host.pid, position)
            .into_iter()
            .collect();
        let context = app.state::<Context>();
        let Ok(_operation) = context.operation.try_lock() else {
            return Ok(Vec::new());
        };
        if lock(&context.scroll)?.is_some() || lock(&context.editor)?.exit_pending {
            return Ok(Vec::new());
        }
        let frames = lock(&context.frames)?;
        let Some(frame) = frames.get(&label) else {
            return Ok(Vec::new());
        };
        if !Arc::ptr_eq(&revision, &frame.element_generation)
            || revision.load(Ordering::Acquire) != generation
            || frame.element_point != Some(point)
            || frame.monitor != monitor
            || !frame.hosts.contains(&host)
        {
            return Ok(Vec::new());
        }
        Ok(bounds
            .into_iter()
            .filter_map(|bounds| {
                crate::window_regions::element_in_image(
                    bounds,
                    (monitor.x, monitor.y),
                    (monitor.width, monitor.height),
                    (monitor.image_width, monitor.image_height),
                    host.rect,
                    point,
                )
            })
            .map(|rect| Region {
                rect,
                label: "控件".into(),
            })
            .collect())
    })
    .await
    .map_err(|_| "控件边界查询失败")?
}
#[tauri::command]
async fn finish_capture(
    app: AppHandle,
    window: WebviewWindow,
    rect: Rect,
    action: Option<CaptureAction>,
    data_url: Option<String>,
) -> Result<CaptureOutcome> {
    let label = window.label().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        let _operation = lock(&context.operation)?;
        context.ensure_not_exiting()?;
        app.state::<Updates>().ensure_idle()?;
        let action = action.unwrap_or_default();
        if lock(&context.scroll)?.is_some() {
            return Err("请使用长截图操作栏完成截图".into());
        }
        let image = {
            let frames = lock(&context.frames)?;
            let frame = frames.get(&label).ok_or("截图会话已结束")?;
            imaging::capture_selection(&frame.image, rect, data_url.as_deref())?
        };
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let name = format!("星镜截图-{seconds}.png");
        // Encode once for recent-image state and any already-running editor. Copy and
        // save operate on the decoded crop rather than decoding this PNG again.
        let result = match data_url {
            Some(data_url) => ImagePayload {
                data_url,
                width: image.width(),
                height: image.height(),
                name,
            },
            None => imaging::payload(&image, name)?,
        };
        complete_capture(&app, &window, image, result, action)
    })
    .await
    .map_err(|_| "裁切任务异常")?
}

// Positions are local logical points; the original monitor origin is applied per OS.
fn scroll_panel_bounds(monitor: CaptureMonitor, rect: Rect) -> (f64, f64, f64, f64) {
    #[cfg(target_os = "macos")]
    let (sw, sh) = (f64::from(monitor.width), f64::from(monitor.height));
    #[cfg(not(target_os = "macos"))]
    let (sw, sh) = (
        f64::from(monitor.width) / monitor.scale,
        f64::from(monitor.height) / monitor.scale,
    );
    let width = 380.0_f64.min((sw - 16.0).max(1.0));
    let height = 300.0_f64.min((sh - 32.0).max(1.0));
    let x = f64::from(rect.x) / f64::from(monitor.image_width) * sw;
    let y = f64::from(rect.y) / f64::from(monitor.image_height) * sh;
    let right = f64::from(rect.x + rect.width) / f64::from(monitor.image_width) * sw;
    let bottom = f64::from(rect.y + rect.height) / f64::from(monitor.image_height) * sh;
    let candidates = [
        (right + 8.0, y),
        (x - width - 8.0, y),
        (x, bottom + 8.0),
        (x, y - height - 8.0),
    ];
    let (px, py) = candidates
        .into_iter()
        .find(|(x, y)| {
            *x >= 8.0 && *y >= 16.0 && *x + width <= sw - 8.0 && *y + height <= sh - 16.0
        })
        .unwrap_or(((sw - width - 8.0).max(0.0), (sh - height - 16.0).max(0.0)));
    (px, py, width, height)
}

#[tauri::command]
async fn begin_scroll_capture(
    app: AppHandle,
    window: WebviewWindow,
    rect: Rect,
) -> Result<ScrollProgress> {
    let label = window.label().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        let _operation = lock(&context.operation)?;
        context.ensure_not_exiting()?;
        app.state::<Updates>().ensure_idle()?;
        if lock(&context.scroll)?.is_some() {
            return Err("已有长截图正在进行".into());
        }
        let scroll = {
            let frames = lock(&context.frames)?;
            let frame = frames.get(&label).ok_or("截图会话已结束")?;
            let image = imaging::capture_selection(&frame.image, rect, None)?.into_rgba8();
            ScrollCapture {
                label: label.clone(),
                monitor: frame.monitor,
                rect,
                stitch: ScrollImage::new(image)?,
            }
        };
        let progress = scroll
            .progress("首屏已保留。请在原页面向下滚动，保留至少三分之一重叠，再点采集下一屏。")?;
        let peers: Vec<_> = lock(&context.frames)?
            .keys()
            .filter(|key| **key != label)
            .filter_map(|key| app.get_webview_window(key))
            .collect();
        let old_position = window.outer_position().map_err(|e| e.to_string())?;
        let old_size = window.inner_size().map_err(|e| e.to_string())?;
        let (x, y, width, height) = scroll_panel_bounds(scroll.monitor, rect);
        let resized = (|| -> Result<()> {
            for peer in &peers {
                peer.hide().map_err(|e| e.to_string())?;
            }
            window.hide().map_err(|e| e.to_string())?;
            #[cfg(target_os = "macos")]
            window
                .set_position(tauri::LogicalPosition::new(
                    f64::from(scroll.monitor.x) + x,
                    f64::from(scroll.monitor.y) + y,
                ))
                .map_err(|e| e.to_string())?;
            #[cfg(not(target_os = "macos"))]
            window
                .set_position(tauri::PhysicalPosition::new(
                    scroll.monitor.x + (x * scroll.monitor.scale).round() as i32,
                    scroll.monitor.y + (y * scroll.monitor.scale).round() as i32,
                ))
                .map_err(|e| e.to_string())?;
            window
                .set_size(tauri::LogicalSize::new(width, height))
                .map_err(|e| e.to_string())?;
            window
                .set_title("星镜 · 滚动长截图")
                .map_err(|e| e.to_string())?;
            window.show().map_err(|e| e.to_string())?;
            window.set_focus().map_err(|e| e.to_string())?;
            Ok(())
        })();
        if let Err(error) = resized {
            let _ = window.set_position(old_position);
            let _ = window.set_size(old_size);
            let _ = window.set_title("星镜 · 选择截图区域");
            for peer in peers {
                let _ = peer.show();
            }
            let _ = window.show();
            let _ = window.set_focus();
            return Err(error);
        }
        *lock(&context.scroll)? = Some(scroll);
        {
            let mut frames = lock(&context.frames)?;
            frames.retain(|key, _| *key == label);
            // Keep only the active label/monitor for lifecycle guards. The frozen
            // desktop and its base64 are no longer needed once scrolling begins.
            if let Some(frame) = frames.get_mut(&label) {
                frame.image = DynamicImage::new_rgba8(1, 1);
                frame.payload.data_url = String::new();
                frame.payload.regions.clear();
            }
        }
        for peer in peers {
            let _ = peer.destroy();
        }
        Ok(progress)
    })
    .await
    .map_err(|_| "开始长截图失败")?
}

#[tauri::command]
async fn append_scroll_capture(app: AppHandle, window: WebviewWindow) -> Result<ScrollProgress> {
    let label = window.label().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        let _operation = lock(&context.operation)?;
        context.ensure_not_exiting()?;
        app.state::<Updates>().ensure_idle()?;
        let (expected, rect) = {
            let scroll = lock(&context.scroll)?;
            let scroll = scroll
                .as_ref()
                .filter(|scroll| scroll.label == label)
                .ok_or("长截图会话已结束")?;
            (scroll.monitor, scroll.rect)
        };
        if !capture_permission(false) {
            return Err(CAPTURE_PERMISSION_HELP.into());
        }
        window.hide().map_err(|e| e.to_string())?;
        // Explicit per-click capture: the panel is removed by the compositor before
        // sampling. There is no automatic scrolling, polling, or background capture.
        std::thread::sleep(Duration::from_millis(180));
        let captured = (|| -> Result<image::RgbaImage> {
            let monitor = xcap::Monitor::all()
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|monitor| monitor.id().ok() == Some(expected.id))
                .ok_or("原显示器已断开，请保存已有长图后重新截图")?;
            let current = CaptureMonitor {
                id: expected.id,
                x: monitor.x().map_err(|e| e.to_string())?,
                y: monitor.y().map_err(|e| e.to_string())?,
                width: monitor.width().map_err(|e| e.to_string())?,
                height: monitor.height().map_err(|e| e.to_string())?,
                scale: f64::from(monitor.scale_factor().map_err(|e| e.to_string())?),
                image_width: expected.image_width,
                image_height: expected.image_height,
            };
            if current != expected {
                return Err("显示器位置、分辨率或缩放已变化，已保留长图，请保存后重新截图".into());
            }
            let image = monitor
                .capture_image()
                .map_err(|e| format!("采集失败，已保留长图：{e}"))?;
            if image.dimensions() != (expected.image_width, expected.image_height) {
                return Err("采集画面尺寸已变化，已保留长图，请保存后重新截图".into());
            }
            rect.validate(image.width(), image.height())?;
            Ok(
                image::imageops::crop_imm(&image, rect.x, rect.y, rect.width, rect.height)
                    .to_image(),
            )
        })();
        let restored = window
            .show()
            .and_then(|_| window.set_focus())
            .map_err(|e| e.to_string());
        let image = captured?;
        restored?;
        let mut scroll = lock(&context.scroll)?;
        let scroll = scroll
            .as_mut()
            .filter(|scroll| scroll.label == label)
            .ok_or("长截图会话已结束")?;
        let added = scroll.stitch.append(image)?;
        scroll.progress(if added {
            "已拼接下一屏。可继续向下滚动采集，或保存当前长图。"
        } else {
            "画面没有变化，未重复拼接。到达底部时可直接保存。"
        })
    })
    .await
    .map_err(|_| "采集长截图失败")?
}

#[tauri::command]
async fn finish_scroll_capture(
    app: AppHandle,
    window: WebviewWindow,
    action: CaptureAction,
) -> Result<CaptureOutcome> {
    let label = window.label().to_string();
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        let _operation = lock(&context.operation)?;
        context.ensure_not_exiting()?;
        app.state::<Updates>().ensure_idle()?;
        let image = {
            let scroll = lock(&context.scroll)?;
            let scroll = scroll
                .as_ref()
                .filter(|scroll| scroll.label == label)
                .ok_or("长截图会话已结束")?;
            DynamicImage::ImageRgba8(scroll.stitch.image().clone())
        };
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let result = imaging::payload(&image, format!("星镜长截图-{seconds}.png"))?;
        complete_capture(&app, &window, image, result, action)
    })
    .await
    .map_err(|_| "完成长截图失败")?
}

// Caller holds Context::operation and has checked lifecycle/update gates.
fn complete_capture(
    app: &AppHandle,
    window: &WebviewWindow,
    image: DynamicImage,
    result: ImagePayload,
    action: CaptureAction,
) -> Result<CaptureOutcome> {
    let context = app.state::<Context>();
    let mut path = None;
    match action {
        CaptureAction::Edit => {}
        CaptureAction::Copy => copy_decoded_image(image)?,
        CaptureAction::Save => {
            // A topmost capture overlay must not cover its native save dialog.
            window.set_always_on_top(false).map_err(|e| e.to_string())?;
            let saved = save_decoded_image(&app, &window, &image, &result.name);
            let restored = window.set_always_on_top(true).map_err(|e| e.to_string());
            path = saved?;
            if path.is_none() {
                restored?;
                let _ = window.set_focus();
                return Ok(CaptureOutcome {
                    completed: false,
                    path: None,
                });
            }
        }
        CaptureAction::Pin => create_pin_payload(&app, result.clone())?,
    }
    *lock(&context.current)? = Some(result.clone());
    clear_capture(&app)?;
    if action == CaptureAction::Edit {
        show_main(&app);
    }
    // Quick actions keep an existing editor's recent images in sync without
    // opening it. A newly created editor obtains current through its startup IPC.
    if app.get_webview_window("main").is_some() {
        let _ = app.emit_to("main", "capture-complete", result);
    }
    Ok(CaptureOutcome {
        completed: true,
        path,
    })
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
        // Dismissing capture keeps the editor tucked away; its session is untouched.
        clear_capture(&app)
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
        context.ensure_not_exiting()?;
        app.state::<Updates>().ensure_idle()?;
        let image = imaging::decode_data_url(&data_url)?;
        save_decoded_image(&app, &window, &image, &suggested_name)
    })
    .await
    .map_err(|_| "保存任务异常")?
}
// Caller holds Context::operation and has checked lifecycle/update gates.
fn save_decoded_image(
    app: &AppHandle,
    window: &WebviewWindow,
    image: &DynamicImage,
    suggested_name: &str,
) -> Result<Option<String>> {
    let name = imaging::filename(suggested_name);
    let file = app
        .dialog()
        .file()
        .set_parent(window)
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
                *channel = ((u32::from(*channel) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
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
    *lock(&app.state::<Context>().last_saved)? = Some(path.clone());
    Ok(Some(path.to_string_lossy().into_owned()))
}
#[tauri::command]
async fn copy_image(app: AppHandle, window: WebviewWindow, data_url: String) -> Result<()> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        let _operation = lock(&context.operation)?;
        context.ensure_not_exiting()?;
        app.state::<Updates>().ensure_idle()?;
        copy_decoded_image(imaging::decode_data_url(&data_url)?)
    })
    .await
    .map_err(|_| "复制图片任务异常")?
}
fn copy_decoded_image(image: DynamicImage) -> Result<()> {
    let image = image.into_rgba8();
    arboard::Clipboard::new()
        .map_err(|e| format!("无法打开剪贴板：{e}"))?
        .set_image(arboard::ImageData {
            width: image.width() as usize,
            height: image.height() as usize,
            bytes: Cow::Owned(image.into_raw()),
        })
        .map_err(|e| format!("复制图片失败：{e}"))
}
#[tauri::command]
async fn copy_text(app: AppHandle, window: WebviewWindow, text: String) -> Result<()> {
    main_only(&window)?;
    if text.trim().is_empty() || text.len() > 8192 {
        return Err("复制文字应为非空且不超过 8 KiB".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        let _operation = lock(&context.operation)?;
        context.ensure_not_exiting()?;
        app.state::<Updates>().ensure_idle()?;
        arboard::Clipboard::new()
            .and_then(|mut clipboard| clipboard.set_text(text))
            .map_err(|e| format!("文字复制失败：{e}"))
    })
    .await
    .map_err(|_| "文字复制任务异常")?
}
#[tauri::command]
fn reveal_saved_image(app: AppHandle, window: WebviewWindow) -> Result<()> {
    main_only(&window)?;
    let path = lock(&app.state::<Context>().last_saved)?
        .clone()
        .ok_or("本次运行尚未成功保存图片")?;
    if !path.is_file() {
        return Err("已保存的图片已被移动或删除".into());
    }
    #[cfg(target_os = "macos")]
    let child = std::process::Command::new("/usr/bin/open")
        .arg("-R")
        .arg(&path)
        .spawn();
    #[cfg(target_os = "windows")]
    let child = std::process::Command::new("explorer.exe")
        .arg(format!("/select,{}", path.to_string_lossy()))
        .spawn();
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let child = std::process::Command::new("xdg-open")
        .arg(path.parent().ok_or("图片位置无效")?)
        .spawn();
    child
        .map(|_| ())
        .map_err(|e| format!("无法打开图片位置：{e}"))
}
// Transparent logical-pixel margin for the preview-only halo. Returned to the
// pin WebView so its content size matches creation, resizing and zooming.
const PIN_GUTTER: f64 = 12.0;

fn create_pin(app: &AppHandle, data_url: &str) -> Result<()> {
    let context = app.state::<Context>();
    let _operation = lock(&context.operation)?;
    context.ensure_not_exiting()?;
    app.state::<Updates>().ensure_idle()?;
    let image = imaging::decode_data_url(data_url)?;
    create_pin_payload(
        app,
        ImagePayload {
            data_url: data_url.to_owned(),
            width: image.width(),
            height: image.height(),
            name: "星镜贴图".into(),
        },
    )
}
// The payload has already been decoded/validated by the caller. Keeping the
// operation lock at the entry points also permits capture -> pin without reentry.
fn create_pin_payload(app: &AppHandle, payload: ImagePayload) -> Result<()> {
    let context = app.state::<Context>();
    let mut pins = lock(&context.pins)?;
    if pins.len() >= 12 {
        return Err("最多同时保留 12 张贴图，请先关闭一些贴图".into());
    }
    let id = context.sequence.fetch_add(1, Ordering::Relaxed);
    let label = format!("pin-{id}");
    let (width, height) = (payload.width, payload.height);
    pins.insert(label.clone(), payload);
    drop(pins);
    let scale = (800.0 / f64::from(width))
        .min(650.0 / f64::from(height))
        .min(1.0);
    let result =
        WebviewWindowBuilder::new(app, &label, WebviewUrl::App("index.html?view=pin".into()))
            .title("星镜 · 桌面贴图")
            .inner_size(
                (f64::from(width) * scale).max(160.0) + 2.0 * PIN_GUTTER,
                (f64::from(height) * scale + 44.0).max(100.0) + 2.0 * PIN_GUTTER,
            )
            .min_inner_size(100.0 + 2.0 * PIN_GUTTER, 80.0 + 2.0 * PIN_GUTTER)
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
fn get_pin_image(app: AppHandle, window: WebviewWindow) -> Result<PinImage> {
    let context = app.state::<Context>();
    let image = lock(&context.pins)?
        .get(window.label())
        .cloned()
        .ok_or("贴图不存在")?;
    let pin_halo = lock(&context.settings)?.pin_halo;
    Ok(PinImage {
        image,
        pin_halo,
        focused: window.is_focused().unwrap_or(false),
        gutter: PIN_GUTTER,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PinImage {
    #[serde(flatten)]
    image: ImagePayload,
    pin_halo: bool,
    focused: bool,
    gutter: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum PinExportAction {
    Copy,
    Save,
    Edit,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PinExportOutcome {
    completed: bool,
    path: Option<String>,
    warning: Option<String>,
}

const PIN_EDIT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PinEditRequest {
    request_id: u64,
    expires_at: u64,
    image: ImagePayload,
}
struct PendingPinEdit {
    request_id: u64,
    expires_at: u64,
    image: Option<ImagePayload>,
    completion: mpsc::Sender<Result<()>>,
}
impl PendingPinEdit {
    fn take(&mut self) -> Option<PinEditRequest> {
        Some(PinEditRequest {
            request_id: self.request_id,
            expires_at: self.expires_at,
            image: self.image.take()?,
        })
    }
}
fn complete_pin_edit(context: &Context, request_id: u64, result: Result<()>) -> Result<()> {
    let mut pending = lock(&context.pending_pin_edit)?;
    if pending.as_ref().map(|request| request.request_id) != Some(request_id) {
        return Err("贴图发送请求已结束；贴图保留".into());
    }
    let request = pending.take().unwrap();
    let _ = request.completion.send(result);
    Ok(())
}
#[tauri::command]
fn take_pending_pin_edit(app: AppHandle, window: WebviewWindow) -> Result<Option<PinEditRequest>> {
    main_only(&window)?;
    let context = app.state::<Context>();
    let request = lock(&context.pending_pin_edit)?
        .as_mut()
        .and_then(PendingPinEdit::take);
    Ok(request)
}
#[tauri::command]
fn finish_pin_edit(
    app: AppHandle,
    window: WebviewWindow,
    request_id: u64,
    error: Option<String>,
) -> Result<()> {
    main_only(&window)?;
    let result = error.map_or(Ok(()), |message| {
        Err(format!(
            "{}；贴图保留",
            message.chars().take(300).collect::<String>()
        ))
    });
    complete_pin_edit(&app.state::<Context>(), request_id, result)
}

#[tauri::command]
async fn pin_export(
    app: AppHandle,
    window: WebviewWindow,
    action: PinExportAction,
    rotation: u16,
    flipped: bool,
) -> Result<PinExportOutcome> {
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        let _operation = lock(&context.operation)?;
        context.ensure_not_exiting()?;
        app.state::<Updates>().ensure_idle()?;
        no_active_capture(&context)?;
        // The caller chooses only an orientation, never a pin id or image bytes.
        let source = lock(&context.pins)?
            .get(window.label())
            .cloned()
            .ok_or("贴图不存在")?;
        let image = imaging::orient_pin(
            imaging::decode_data_url(&source.data_url)?,
            rotation,
            flipped,
        )?;
        let mut outcome = PinExportOutcome {
            completed: true,
            path: None,
            warning: None,
        };
        match action {
            PinExportAction::Copy => copy_decoded_image(image)?,
            PinExportAction::Save => {
                window.set_always_on_top(false).map_err(|e| e.to_string())?;
                let saved = save_decoded_image(&app, &window, &image, &source.name);
                let restored = window.set_always_on_top(true);
                outcome.path = saved?;
                outcome.completed = outcome.path.is_some();
                if let Err(error) = restored {
                    outcome.warning = Some(format!("贴图恢复置顶失败：{error}"));
                }
            }
            PinExportAction::Edit => {
                if u64::from(image.width()) * u64::from(image.height())
                    > imaging::MAX_CAPTURE_PIXELS
                {
                    return Err("编辑器最多接收 4000 万像素，贴图仍保留；可直接复制或另存为".into());
                }
                let result = imaging::payload(&image, source.name)?;
                // Encoding can take long enough for the editor to enter backup,
                // restore or another image operation. Check its state again here.
                context.ensure_not_exiting()?;
                app.state::<Updates>().ensure_idle()?;
                if app.get_webview_window("main").is_some() {
                    let editor = lock(&context.editor)?;
                    if !editor.reported || editor.busy {
                        return Err("编辑器正在处理图片，请稍后重试；贴图保留".into());
                    }
                }
                let request_id = context.sequence.fetch_add(1, Ordering::SeqCst) + 1;
                let (completion, received) = mpsc::channel();
                {
                    // ponytail: one outstanding pin delivery is enough; avoid a
                    // queue of large image payloads while the editor is occupied.
                    let mut pending = lock(&context.pending_pin_edit)?;
                    if pending.is_some() {
                        return Err("另一张贴图正在发送，请稍后重试；贴图保留".into());
                    }
                    *pending = Some(PendingPinEdit {
                        request_id,
                        expires_at: SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map_err(|_| "系统时间异常")?
                            .as_millis() as u64
                            + PIN_EDIT_TIMEOUT.as_millis() as u64,
                        image: Some(result),
                        completion,
                    });
                }
                // The editor must be able to report state and finish its other
                // work while it receives this request. Never wait with this lock.
                drop(_operation);
                let handle = app.clone();
                if let Err(error) = app.run_on_main_thread(move || {
                    let result = show_main_window(&handle).and_then(|()| {
                        handle
                            .emit_to("main", "pin-edit-requested", ())
                            .map_err(|error| error.to_string())
                    });
                    if let Err(error) = result {
                        let _ = complete_pin_edit(
                            &handle.state::<Context>(),
                            request_id,
                            Err(format!("无法打开编辑器：{error}；贴图保留")),
                        );
                    }
                }) {
                    let _ = complete_pin_edit(
                        &context,
                        request_id,
                        Err(format!("无法打开编辑器：{error}；贴图保留")),
                    );
                }
                match received.recv_timeout(PIN_EDIT_TIMEOUT) {
                    Ok(result) => result?,
                    Err(_) => {
                        let message = "编辑器接收超时，请稍后重试；贴图保留";
                        let _ = complete_pin_edit(&context, request_id, Err(message.into()));
                        return Err(message.into());
                    }
                }
            }
        }
        Ok(outcome)
    })
    .await
    .map_err(|_| "贴图输出任务异常")?
}
#[tauri::command]
fn pin_action(
    app: AppHandle,
    window: WebviewWindow,
    action: String,
    value: Option<f64>,
) -> Result<()> {
    let context = app.state::<Context>();
    // This synchronous UI command must not block the main thread while another
    // pin's save dialog is using the operation lock on a worker thread.
    let _operation = context
        .operation
        .try_lock()
        .map_err(|_| "正在处理图片，请稍后重试")?;
    context.ensure_not_exiting()?;
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
        "halo" => {
            let enabled = match value {
                Some(0.0) => false,
                Some(1.0) => true,
                _ => return Err("贴图光圈开关无效".into()),
            };
            let directory = app.path().app_data_dir().map_err(|e| e.to_string())?;
            let mut settings = lock(&context.settings)?;
            let next = DesktopSettings {
                pin_halo: enabled,
                ..settings.clone()
            };
            desktop_settings::save(&directory, &next)?;
            *settings = next;
            drop(settings);
            let _ = app.emit("pin-halo", enabled);
            Ok(())
        }
        "opacity" => {
            let opacity = value
                .filter(|v| v.is_finite() && (0.15..=1.0).contains(v))
                .ok_or("透明度应为 15%–100%")?;
            window
                .emit_to(window.label(), "pin-opacity", opacity)
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
                    (f64::from(image.width) * base * scale).clamp(100.0, 8000.0) + 2.0 * PIN_GUTTER,
                    (f64::from(image.height) * base * scale + 44.0).clamp(80.0, 8000.0)
                        + 2.0 * PIN_GUTTER,
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
    element_detection: bool,
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
        element_detection: crate::window_regions::element_detection_available(),
        shortcut: lock(&app.state::<Context>().settings)?
            .capture_shortcut
            .clone(),
        error: shortcut_error.or_else(|| {
            if permission {
                None
            } else {
                Some(CAPTURE_PERMISSION_HELP.into())
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
fn get_desktop_settings(app: AppHandle, window: WebviewWindow) -> Result<DesktopSettingsStatus> {
    main_only(&window)?;
    Ok(DesktopSettingsStatus {
        preferences: lock(&app.state::<Context>().settings)?.clone(),
        autostart: app
            .autolaunch()
            .is_enabled()
            .map_err(|e| format!("无法读取系统开机启动状态：{e}"))?,
    })
}
#[tauri::command]
fn set_desktop_settings(
    app: AppHandle,
    window: WebviewWindow,
    settings: DesktopSettingsStatus,
) -> Result<DesktopSettingsStatus> {
    main_only(&window)?;
    let context = app.state::<Context>();
    let _operation = context
        .operation
        .try_lock()
        .map_err(|_| "另一项编辑操作仍在进行")?;
    context.ensure_not_exiting()?;
    app.state::<Updates>().ensure_idle()?;
    no_active_capture(&context)?;
    let directory = app.path().app_data_dir().map_err(|e| e.to_string())?;
    let mut old = lock(&context.settings)?;
    let previous_autostart = app
        .autolaunch()
        .is_enabled()
        .map_err(|e| format!("无法读取系统开机启动状态：{e}"))?;
    desktop_settings::replace_shortcuts(
        &old,
        &settings.preferences,
        |shortcut, register| {
            if register && app.global_shortcut().is_registered(shortcut) {
                Ok(false)
            } else if register {
                app.global_shortcut().register(shortcut).map(|_| true)
            } else if app.global_shortcut().is_registered(shortcut) {
                app.global_shortcut().unregister(shortcut).map(|_| true)
            } else {
                Ok(false)
            }
            .map_err(|e| e.to_string())
        },
        || {
            desktop_settings::update_startup(
                previous_autostart,
                settings.autostart,
                |enabled| {
                    let launcher = app.autolaunch();
                    if enabled {
                        launcher.enable()
                    } else {
                        launcher.disable()
                    }
                    .map_err(|e| e.to_string())?;
                    if launcher.is_enabled().map_err(|e| e.to_string())? != enabled {
                        return Err("系统启动项状态未匹配请求，请检查系统设置".into());
                    }
                    Ok(())
                },
                || desktop_settings::save(&directory, &settings.preferences),
            )
        },
    )?;
    *old = settings.preferences.clone();
    drop(old);
    *lock(&context.shortcut_error)? = None;
    let _ = app.emit("pin-halo", settings.preferences.pin_halo);
    Ok(settings)
}
#[tauri::command]
fn set_editor_state(
    app: AppHandle,
    window: WebviewWindow,
    has_session: bool,
    dirty: bool,
    busy: bool,
) -> Result<()> {
    main_only(&window)?;
    let (pending, release_startup) = {
        let context = app.state::<Context>();
        let mut editor = lock(&context.editor)?;
        editor.update(has_session, dirty, busy);
        let announce = editor.exit_pending && !editor.exit_announced;
        if announce {
            editor.exit_announced = true;
        }
        let release = editor.release_startup_when_ready && editor.empty_idle();
        if release {
            editor.release_startup_when_ready = false;
        }
        (announce, release)
    };
    if pending {
        app.emit_to("main", "request-exit", ())
            .map_err(|e| e.to_string())?;
    }
    if release_startup {
        offer_idle_release(&app);
    }
    Ok(())
}
fn begin_exit(app: &AppHandle) -> Result<()> {
    let context = app.state::<Context>();
    let _operation = context
        .operation
        .try_lock()
        .map_err(|_| "正在处理图片，请完成后再退出")?;
    app.state::<Updates>().ensure_idle()?;
    no_active_capture(&context)?;
    let (immediate, announce) = {
        let mut editor = lock(&context.editor)?;
        if editor.exit_approved {
            return Ok(());
        }
        if editor.reported && editor.busy {
            return Err("正在处理编辑，请完成后再退出".into());
        }
        // A cached clean flag can race a new frontend edit. Only a released empty
        // WebView can exit without asking the current frontend to lock and flush.
        let immediate = editor.released;
        let announce = editor.reported && !editor.exit_announced;
        editor.exit_pending = true;
        editor.release_request = None;
        editor.exit_approved = immediate;
        if announce {
            editor.exit_announced = true;
        }
        (immediate, announce)
    };
    if immediate {
        app.exit(0);
    } else {
        show_main(app);
        if announce {
            let _ = app.emit_to("main", "request-exit", ());
        }
    }
    Ok(())
}
#[tauri::command]
fn request_exit(app: AppHandle, window: WebviewWindow) -> Result<()> {
    main_only(&window)?;
    begin_exit(&app)
}
#[tauri::command]
fn cancel_exit(app: AppHandle, window: WebviewWindow) -> Result<()> {
    main_only(&window)?;
    let context = app.state::<Context>();
    let mut editor = lock(&context.editor)?;
    if editor.exit_approved {
        return Err("退出已开始".into());
    }
    editor.exit_pending = false;
    editor.exit_announced = false;
    Ok(())
}
#[tauri::command]
fn confirm_exit(app: AppHandle, window: WebviewWindow) -> Result<()> {
    main_only(&window)?;
    let context = app.state::<Context>();
    let _operation = context
        .operation
        .try_lock()
        .map_err(|_| "正在处理图片，请完成后再退出")?;
    app.state::<Updates>().ensure_idle()?;
    no_active_capture(&context)?;
    lock(&context.editor)?.confirm_exit()?;
    app.exit(0);
    Ok(())
}
fn offer_idle_release(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let context = app.state::<Context>();
    if app.state::<Updates>().ensure_idle().is_err() || no_active_capture(&context).is_err() {
        return;
    }
    if context.current.lock().map_or(true, |image| image.is_some()) {
        return;
    }
    if context
        .pending_pin_edit
        .lock()
        .map_or(true, |request| request.is_some())
    {
        return;
    }
    let request_id = {
        let Ok(mut editor) = context.editor.lock() else {
            return;
        };
        if !editor.empty_idle() {
            return;
        }
        let id = context.sequence.fetch_add(1, Ordering::SeqCst) + 1;
        editor.release_request = Some(id);
        id
    };
    let _ = window.emit(
        "request-idle-release",
        serde_json::json!({"requestId":request_id}),
    );
}
#[tauri::command]
fn cancel_idle_release(app: AppHandle, window: WebviewWindow, request_id: u64) -> Result<()> {
    main_only(&window)?;
    let context = app.state::<Context>();
    let mut editor = lock(&context.editor)?;
    if editor.release_request == Some(request_id) {
        editor.release_request = None;
    }
    Ok(())
}
#[tauri::command]
async fn release_empty_editor(
    app: AppHandle,
    window: WebviewWindow,
    request_id: u64,
) -> Result<()> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        // Initial read-only metadata loading can overlap the ready handshake.
        // Wait off the UI thread, then re-check every condition and request token.
        let _operation = lock(&context.operation)?;
        app.state::<Updates>().ensure_idle()?;
        no_active_capture(&context)?;
        if lock(&context.current)?.is_some() {
            return Err("仍有原生图片待编辑，保留编辑器".into());
        }
        if lock(&context.pending_pin_edit)?.is_some() {
            return Err("仍有贴图待接收，保留编辑器".into());
        }
        lock(&context.editor)?.release(request_id)?;
        if let Err(error) = window.destroy() {
            let mut editor = lock(&context.editor)?;
            editor.released = false;
            editor.reported = true;
            return Err(error.to_string());
        }
        Ok(())
    })
    .await
    .map_err(|_| "释放空编辑器失败")?
}
#[tauri::command]
async fn recognize_text(
    app: AppHandle,
    window: WebviewWindow,
    data_url: String,
) -> Result<crate::ocr::OcrResult> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        let _operation = context
            .operation
            .try_lock()
            .map_err(|_| "另一项图片操作仍在进行")?;
        context.ensure_not_exiting()?;
        app.state::<Updates>().ensure_idle()?;
        no_active_capture(&context)?;
        crate::ocr::recognize(&data_url)
    })
    .await
    .map_err(|_| "本地文字识别异常终止，请重试或手动打码")?
}
#[tauri::command]
async fn get_recovery_storage(app: AppHandle, window: WebviewWindow) -> Result<RecoveryStorage> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        let _operation = lock(&context.operation)?;
        app.state::<Updates>().ensure_idle()?;
        recovery::storage(&app.path().app_data_dir().map_err(|e| e.to_string())?)
    })
    .await
    .map_err(|_| "统计备份空间失败")?
}
#[tauri::command]
async fn delete_recovery_point(app: AppHandle, window: WebviewWindow, id: String) -> Result<()> {
    main_only(&window)?;
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        let _operation = lock(&context.operation)?;
        context.ensure_not_exiting()?;
        app.state::<Updates>().ensure_idle()?;
        no_active_capture(&context)?;
        recovery::delete(&app.path().app_data_dir().map_err(|e| e.to_string())?, &id)
    })
    .await
    .map_err(|_| "删除恢复点失败")?
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
        context.ensure_not_exiting()?;
        no_active_capture(&context)?;
        app.state::<Updates>().begin_check(&app)?;
    }
    let before_exit = app.clone();
    let cleanup_started = app.state::<Updates>().cleanup_flag()?;
    let result = async {
        let update = app
            .updater_builder()
            .timeout(Duration::from_secs(30))
            .restart_after_install(true)
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
async fn download_starshot_update(
    app: AppHandle,
    window: WebviewWindow,
    expected_version: String,
) -> Result<UpdateStatus> {
    main_only(&window)?;
    let update = {
        let context = app.state::<Context>();
        let _operation = context
            .operation
            .try_lock()
            .map_err(|_| "另一项编辑操作仍在进行")?;
        context.ensure_not_exiting()?;
        no_active_capture(&context)?;
        app.state::<Updates>()
            .begin_download(&app, &expected_version)?
    };
    let result: Result<()> = async {
        let mut downloaded = 0_u64;
        let mut last_progress = Instant::now();
        // The plugin returns only after signature and signed-version validation.
        // This command never snapshots, installs, or restarts the application.
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
            .downloaded(&app, &expected_version, bytes)
    }
    .await;
    if let Err(error) = result {
        return app.state::<Updates>().failed(&app, error);
    }
    // Foreground the ready confirmation after verification, including from the tray.
    // Window focus is best-effort and must not invalidate the verified cache.
    show_main(&app);
    app.state::<Updates>().status()
}
#[tauri::command]
fn set_update_confirmation(
    app: AppHandle,
    window: WebviewWindow,
    expected_version: String,
    pending: bool,
) -> Result<UpdateStatus> {
    main_only(&window)?;
    let context = app.state::<Context>();
    let _operation = context
        .operation
        .try_lock()
        .map_err(|_| "另一项编辑操作仍在进行")?;
    if pending {
        context.ensure_not_exiting()?;
        no_active_capture(&context)?;
    }
    app.state::<Updates>()
        .set_confirmation(&app, &expected_version, pending)?;
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
    // Snapshot the latest confirmed editor session; downloads do not capture stale edits.
    let session = tauri::async_runtime::spawn_blocking(move || {
        recovery::validate_session(&session)?;
        Ok::<_, String>(session)
    })
    .await
    .map_err(|_| "检查编辑会话失败")??;
    let (update, bytes, cleanup_started) = {
        let context = app.state::<Context>();
        let _operation = context
            .operation
            .try_lock()
            .map_err(|_| "另一项编辑操作仍在进行")?;
        context.ensure_not_exiting()?;
        no_active_capture(&context)?;
        app.state::<Updates>()
            .begin_install(&app, &expected_version)?
    };
    // Cache process-only restart data before cleanup invalidates Tauri resources.
    let restart_env = app.env();
    let result: Result<()> = async {
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
            let install_result = update.install(bytes.as_slice());
            // Windows success exits inside the updater. A returned failure after its
            // cleanup hook cannot safely emit events or reuse hidden webviews.
            if worker_cleanup_started.load(Ordering::SeqCst) {
                tauri::process::restart(&worker_restart_env);
            }
            install_result
                .map_err(|error| format!("安装未完成，旧版应用和编辑会话恢复点已保留：{error}"))?;
            worker_app.state::<Updates>().installed(&worker_app)?;
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
    // macOS reaches here; Windows exits and relaunches via its installer.
    app.restart();
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
    let directory = app
        .path()
        .app_data_dir()
        .map_err(|error| error.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let context = app.state::<Context>();
        let _operation = lock(&context.operation)?;
        context.ensure_not_exiting()?;
        app.state::<Updates>().ensure_idle()?;
        no_active_capture(&context)?;
        recovery::read(&directory, &id)
    })
    .await
    .map_err(|_| "读取恢复点失败")?
}
#[tauri::command]
fn restart_app(app: AppHandle, window: WebviewWindow) -> Result<()> {
    main_only(&window)?;
    app.state::<Updates>().allow_restart()?;
    app.restart();
}

#[cfg(target_os = "macos")]
fn install_guarded_app_menu(app: &AppHandle) -> tauri::Result<()> {
    use tauri::menu::{AboutMetadata, PredefinedMenuItem, Submenu};

    // macOS's predefined Quit calls NSApplication terminate: directly, bypassing
    // Tauri's ExitRequested event. Keep the other default menus and route Cmd+Q
    // through the same frontend backup handshake as the tray's exit action.
    let menu = Menu::default(app)?;
    let quit = MenuItem::with_id(app, "request-app-exit", "退出星镜", true, Some("Cmd+Q"))?;
    let application = Submenu::with_items(
        app,
        "Starshot",
        true,
        &[
            &PredefinedMenuItem::about(
                app,
                Some("关于星镜"),
                Some(AboutMetadata {
                    name: Some("Starshot".into()),
                    version: Some(app.package_info().version.to_string()),
                    ..Default::default()
                }),
            )?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::services(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, Some("隐藏星镜"))?,
            &PredefinedMenuItem::hide_others(app, Some("隐藏其他"))?,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    // Tauri's macOS default menu puts the application submenu first.
    menu.remove_at(0)?;
    menu.insert(&application, 0)?;
    app.on_menu_event(|app, event| {
        if event.id.as_ref() == "request-app-exit" {
            if let Err(error) = begin_exit(app) {
                report_error(app, &error);
            }
        }
    });
    app.set_menu(menu)?;
    Ok(())
}

pub fn run() {
    tauri::Builder::default()
        .manage(Context::default())
        .manage(Updates::new())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name("Starshot")
                .arg("--start-in-tray")
                .build(),
        )
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
                    let bindings = app
                        .state::<Context>()
                        .settings
                        .lock()
                        .ok()
                        .and_then(|settings| settings.shortcuts().ok());
                    if let Some([capture, pin]) = bindings {
                        if &capture == shortcut {
                            dispatch_capture(app);
                        } else if &pin == shortcut {
                            dispatch_pin(app);
                        }
                    }
                })
                .build(),
        )
        .setup(|app| {
            #[cfg(target_os = "macos")]
            install_guarded_app_menu(app.handle())?;
            let mut errors = Vec::new();
            let directory = app.path().app_data_dir()?;
            let settings = desktop_settings::load(&directory).unwrap_or_else(|error| {
                errors.push(error);
                DesktopSettings::default()
            });
            for shortcut in settings.shortcuts()? {
                if let Err(error) = app.global_shortcut().register(shortcut) {
                    errors.push(format!("快捷键注册失败：{error}"));
                }
            }
            let hidden =
                settings.start_in_tray || std::env::args().any(|arg| arg == "--start-in-tray");
            *lock(&app.state::<Context>().settings)? = settings;
            lock(&app.state::<Context>().editor)?.release_startup_when_ready = hidden;
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
                        if let Err(error) = begin_exit(app) {
                            report_error(app, &error);
                        }
                    }
                    _ => {}
                });
            tray.build(app)?;
            if !hidden {
                show_main(app.handle());
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    api.prevent_close();
                    let _ = window.hide();
                    offer_idle_release(window.app_handle());
                } else if window.label().starts_with("capture-") {
                    api.prevent_close();
                    let app = window.app_handle().clone();
                    let label = window.label().to_string();
                    tauri::async_runtime::spawn_blocking(move || {
                        let context = app.state::<Context>();
                        if let Ok(_operation) = context.operation.lock() {
                            // A queued close after a successful quick action is stale.
                            let active = context
                                .frames
                                .lock()
                                .is_ok_and(|frames| frames.contains_key(&label));
                            if active {
                                let _ = clear_capture(&app);
                            }
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
            capture_overlay_ready,
            get_capture_element,
            invalidate_capture_element,
            finish_capture,
            begin_scroll_capture,
            append_scroll_capture,
            finish_scroll_capture,
            cancel_capture,
            get_current_image,
            save_image,
            copy_image,
            pin_image,
            get_pin_image,
            pin_action,
            pin_export,
            take_pending_pin_edit,
            finish_pin_edit,
            get_app_status,
            get_update_status,
            check_starshot_update,
            download_starshot_update,
            set_update_confirmation,
            install_starshot_update,
            list_recovery_points,
            create_recovery_point,
            read_recovery_point,
            restart_app,
            get_desktop_settings,
            set_desktop_settings,
            set_editor_state,
            request_exit,
            cancel_exit,
            confirm_exit,
            cancel_idle_release,
            release_empty_editor,
            recognize_text,
            get_recovery_storage,
            delete_recovery_point,
            copy_text,
            reveal_saved_image
        ])
        .build(tauri::generate_context!())
        .expect("星镜无法启动")
        .run(|app, event| {
            match event {
                tauri::RunEvent::ExitRequested { api, code, .. } => {
                    if code == Some(tauri::RESTART_EXIT_CODE) {
                        return;
                    }
                    let approved = app
                        .state::<Context>()
                        .editor
                        .lock()
                        .is_ok_and(|editor| editor.exit_approved);
                    if !approved {
                        api.prevent_exit();
                        // Last-window destruction must leave the tray process alive.
                        if app.get_webview_window("main").is_none() && code.is_none() {
                            return;
                        }
                        if let Err(error) = begin_exit(app) {
                            report_error(app, &error);
                        }
                    }
                }
                #[cfg(target_os = "macos")]
                tauri::RunEvent::Reopen { .. } => show_main(app),
                _ => {}
            }
        });
}

#[cfg(test)]
mod lifecycle_tests {
    use super::{CaptureAction, CaptureOutcome, EditorLifecycle};

    #[test]
    fn capture_action_contract_rejects_unknown_actions_and_reports_save_cancellation() {
        assert_eq!(
            Option::<CaptureAction>::None.unwrap_or_default(),
            CaptureAction::Edit
        );
        for (text, action) in [
            ("edit", CaptureAction::Edit),
            ("copy", CaptureAction::Copy),
            ("save", CaptureAction::Save),
            ("pin", CaptureAction::Pin),
        ] {
            assert_eq!(
                serde_json::from_value::<CaptureAction>(serde_json::json!(text)).unwrap(),
                action
            );
        }
        for invalid in [
            serde_json::json!("upload"),
            serde_json::json!("COPY"),
            serde_json::json!(4),
        ] {
            assert!(serde_json::from_value::<CaptureAction>(invalid).is_err());
        }
        assert_eq!(
            serde_json::to_value(CaptureOutcome {
                completed: false,
                path: None
            })
            .unwrap(),
            serde_json::json!({"completed":false})
        );
    }

    #[test]
    fn unknown_or_busy_editor_cannot_exit_or_release_and_stale_release_is_rejected() {
        let mut state = EditorLifecycle::default();
        assert!(!state.empty_idle());
        assert!(state.confirm_exit().is_err());
        state.update(false, false, false);
        state.release_request = Some(7);
        assert!(state.release(6).is_err());
        state.update(true, true, false);
        assert!(state.release(7).is_err());
        state.exit_pending = true;
        state.update(true, true, true);
        assert!(state.confirm_exit().is_err());
        state.update(true, true, false);
        state.confirm_exit().unwrap();
        assert!(state.exit_approved);
    }

    #[test]
    fn only_confirmed_empty_idle_editor_releases() {
        let mut state = EditorLifecycle::default();
        state.update(false, false, false);
        state.release_request = Some(9);
        state.release(9).unwrap();
        assert!(state.released);
        assert!(!state.reported);
        assert!(state.release(9).is_err());
    }
}

#[cfg(test)]
mod pin_edit_tests {
    use super::{complete_pin_edit, lock, mpsc, Context, ImagePayload, PendingPinEdit};

    #[test]
    fn request_is_taken_once_and_late_receipts_cannot_complete_a_retry() {
        let context = Context::default();
        let (completion, received) = mpsc::channel();
        *lock(&context.pending_pin_edit).unwrap() = Some(PendingPinEdit {
            request_id: 2,
            expires_at: 30_000,
            image: Some(ImagePayload {
                data_url: "original-test-pixels".into(),
                name: "原创测试".into(),
                width: 1,
                height: 1,
            }),
            completion,
        });
        {
            let mut pending = lock(&context.pending_pin_edit).unwrap();
            let request = pending.as_mut().unwrap().take().unwrap();
            assert_eq!(request.request_id, 2);
            assert_eq!(request.expires_at, 30_000);
            assert!(pending.as_mut().unwrap().take().is_none());
        }
        assert!(complete_pin_edit(&context, 1, Ok(())).is_err());
        assert!(received.try_recv().is_err());
        complete_pin_edit(&context, 2, Err("请求已过期".into())).unwrap();
        assert_eq!(received.recv().unwrap(), Err("请求已过期".into()));
        assert!(lock(&context.pending_pin_edit).unwrap().is_none());
        assert!(complete_pin_edit(&context, 2, Ok(())).is_err());
    }
}
