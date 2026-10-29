//! Opt-in acceptance binary mode. It supplies ONLY generated pixels and a caller
//! selected original fixture. Never calls xcap capture, OCR, clipboard or settings.
use super::*;
use crate::window_regions::windows::{self, Reply, Request};
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Clone, Deserialize)]
struct PointCase {
    id: String,
    x: i32,
    y: i32,
    expected: ScreenRect,
}
#[derive(Clone, Deserialize, Serialize)]
struct ScreenRect {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}
#[derive(Clone, Deserialize)]
struct Fixture {
    hwnd: isize,
    pid: u32,
    screen: ScreenRect,
    points: Vec<PointCase>,
}
struct Smoke {
    output: PathBuf,
    point: crate::window_regions::ImagePoint,
    points: Vec<PointCase>,
}

#[tauri::command]
fn native_smoke_move(app: AppHandle, index: usize) -> Result<()> {
    let smoke = app.state::<Smoke>();
    let point = smoke
        .points
        .get(index)
        .ok_or("unknown original fixture point")?;
    // Only predeclared original fixture positions; no clicks or arbitrary coordinates.
    unsafe { ::windows::Win32::UI::WindowsAndMessaging::SetPhysicalCursorPos(point.x, point.y) }
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn native_smoke_report(
    app: AppHandle,
    window: WebviewWindow,
    mut records: Value,
) -> Result<()> {
    unsafe {
        use ::windows::Win32::{System::Threading::GetCurrentProcess, UI::HiDpi::*};
        records["dpi"] = json!({
            "processPmv2": AreDpiAwarenessContextsEqual(GetDpiAwarenessContextForProcess(GetCurrentProcess()), DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).as_bool(),
            "reportThreadPmv2": AreDpiAwarenessContextsEqual(GetThreadDpiAwarenessContext(), DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).as_bool()
        });
    }
    let path = app.state::<Smoke>().output.clone();
    let point = app.state::<Smoke>().point;
    let query_app = app.clone();
    let query_window = window.clone();
    let pending = tauri::async_runtime::spawn(async move {
        get_capture_element(query_app, query_window, point, 50000).await
    });
    let _ = tauri::async_runtime::spawn_blocking(|| std::thread::sleep(Duration::from_millis(60)))
        .await;
    let start = Instant::now();
    cancel_capture(app.clone(), window).await?;
    records["cancelDuringQueryMs"] = json!(start.elapsed().as_millis());
    records["cancelledQueryEmpty"] = json!(pending
        .await
        .ok()
        .and_then(std::result::Result::ok)
        .is_some_and(|v| v.is_empty()));
    records["exitViaProductionGuard"] = json!(true);
    let output = serde_json::to_vec_pretty(&records).map_err(|e| e.to_string())?;
    if output.len() > 64 * 1024 {
        return Err("test record too large".into());
    }
    std::fs::write(path, output).map_err(|e| e.to_string())?;
    begin_exit(&app)?;
    Ok(())
}

pub fn run_if_requested() -> bool {
    let args: Vec<_> = std::env::args().collect();
    let Some(mode) = args.get(1) else {
        return false;
    };
    if mode != "--starshot-uia-probe" && mode != "--starshot-uia-overlay" {
        return false;
    }
    let result = (|| -> Result<()> {
        let input = args.get(2).ok_or("missing fixture path")?;
        let output = PathBuf::from(args.get(3).ok_or("missing output path")?);
        let value: Value =
            serde_json::from_slice(&std::fs::read(input).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let fixture: Fixture = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
        let target = if let Some(saved) = value.get("savedTarget") {
            serde_json::from_value(saved.clone()).map_err(|e| e.to_string())?
        } else {
            windows::describe(fixture.hwnd, 0).ok_or("fixture window unavailable")?
        };
        if target.pid != fixture.pid {
            return Err("fixture identity mismatch".into());
        }
        if mode == "--starshot-uia-probe" {
            let mut rows = Vec::new();
            for point in &fixture.points {
                for repeat in 0..2 {
                    let before = census();
                    let generation = Arc::new(AtomicU64::new(0));
                    let canceller = value
                        .get("cancelAfterMs")
                        .and_then(Value::as_u64)
                        .map(|ms| {
                            let g = generation.clone();
                            std::thread::spawn(move || {
                                std::thread::sleep(Duration::from_millis(ms));
                                g.store(1, Ordering::Release);
                            })
                        });
                    let start = Instant::now();
                    let reply: Reply = windows::query_reply(
                        Request {
                            target,
                            x: point.x,
                            y: point.y,
                        },
                        &generation,
                        0,
                    );
                    if let Some(thread) = canceller {
                        let _ = thread.join();
                    }
                    let wall_ms = start.elapsed().as_millis();
                    std::thread::sleep(Duration::from_millis(10));
                    rows.push(json!({"id":point.id,"repeat":repeat,"expected":point.expected,"reply":reply,"wallMs":wall_ms,"before":before,"after":census()}));
                }
            }
            std::fs::write(
                output,
                serde_json::to_vec_pretty(&json!({"target":target,"rows":rows})).unwrap(),
            )
            .map_err(|e| e.to_string())?;
        } else {
            overlay(fixture, target, output)?;
        }
        Ok(())
    })();
    if let Err(error) = result {
        eprintln!("native acceptance: {error}");
        std::process::exit(1);
    }
    true
}

pub(super) fn census() -> Value {
    use ::windows::Win32::{
        Foundation::CloseHandle,
        System::{
            Diagnostics::ToolHelp::*,
            Threading::{GetCurrentProcess, GetProcessHandleCount},
        },
    };
    unsafe {
        let pid = std::process::id();
        let mut threads = 0;
        let mut children = 0;
        let mut helper_children = 0;
        let mut handles = 0;
        let mut t = THREADENTRY32 {
            dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) {
            let mut more = Thread32First(snapshot, &mut t).is_ok();
            while more {
                if t.th32OwnerProcessID == pid {
                    threads += 1;
                }
                more = Thread32Next(snapshot, &mut t).is_ok();
            }
            let _ = CloseHandle(snapshot);
        }
        let mut p = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
            let mut more = Process32FirstW(snapshot, &mut p).is_ok();
            while more {
                if p.th32ParentProcessID == pid {
                    children += 1;
                    let name = String::from_utf16_lossy(
                        &p.szExeFile[..p
                            .szExeFile
                            .iter()
                            .position(|c| *c == 0)
                            .unwrap_or(p.szExeFile.len())],
                    );
                    if name.eq_ignore_ascii_case("starshot.exe") {
                        helper_children += 1;
                    }
                }
                more = Process32NextW(snapshot, &mut p).is_ok();
            }
            let _ = CloseHandle(snapshot);
        }
        let _ = GetProcessHandleCount(GetCurrentProcess(), &mut handles);
        json!({"threads":threads,"children":children,"helperChildren":helper_children,"handles":handles})
    }
}

fn overlay(
    fixture: Fixture,
    target: crate::window_regions::WindowBounds,
    output: PathBuf,
) -> Result<()> {
    let profile = tempfile::tempdir().map_err(|e| e.to_string())?;
    let mut config = tauri::generate_context!();
    config.config_mut().app.windows.clear();
    let fixture_js = serde_json::to_string(&json!({"screen": {"x":fixture.screen.x,"y":fixture.screen.y,"width":fixture.screen.width,"height":fixture.screen.height},
        "points":fixture.points.iter().map(|p| json!({"id":p.id,"x":p.x-fixture.screen.x,"y":p.y-fixture.screen.y,"expected":p.expected})).collect::<Vec<_>>()})).unwrap();
    let script = include_str!("../../../tests/windows-overlay-driver.js")
        .replace("__FIXTURE_JSON__", &fixture_js);
    let context = Context::default();
    lock(&context.editor)?.released = true; // Original test owns no editor session to discard.
    let app = tauri::Builder::default()
        .manage(context)
        .manage(Updates::new())
        .manage(Smoke {
            output,
            points: fixture.points.clone(),
            point: crate::window_regions::ImagePoint {
                x: (fixture.points[0].x - fixture.screen.x) as u32,
                y: (fixture.points[0].y - fixture.screen.y) as u32,
            },
        })
        .invoke_handler(tauri::generate_handler![
            get_capture_frame,
            capture_overlay_ready,
            get_capture_element,
            invalidate_capture_element,
            cancel_capture,
            native_smoke_move,
            native_smoke_report
        ])
        .setup(move |app| {
            let screen = &fixture.screen;
            let image = DynamicImage::ImageRgba8(image::RgbaImage::from_fn(
                screen.width,
                screen.height,
                |x, y| {
                    image::Rgba(if (x / 80 + y / 80) % 2 == 0 {
                        [26, 32, 46, 255]
                    } else {
                        [36, 44, 60, 255]
                    })
                },
            ));
            let rect = imaging::intersect_region(
                target.x,
                target.y,
                target.width,
                target.height,
                screen.x,
                screen.y,
                1.0,
                screen.width,
                screen.height,
            )
            .ok_or("fixture outside screen")?;
            let label = "capture-native-smoke-0";
            let frame = Frame {
                presented: false,
                payload: FramePayload {
                    data_url: imaging::png_data_url(&image)?,
                    width: screen.width,
                    height: screen.height,
                    regions: vec![Region {
                        rect,
                        label: "窗口".into(),
                    }],
                    scale_factor: 1.0,
                    region_warning: None,
                    pointer: None,
                    element_detection: true,
                },
                image,
                monitor: CaptureMonitor {
                    id: 0,
                    x: screen.x,
                    y: screen.y,
                    width: screen.width,
                    height: screen.height,
                    scale: 1.0,
                    image_width: screen.width,
                    image_height: screen.height,
                },
                hosts: vec![CaptureHost {
                    pid: target.pid,
                    rect,
                    window: target,
                }],
                element_generation: Arc::new(AtomicU64::new(0)),
                element_point: None,
            };
            lock(&app.state::<Context>().frames)?.insert(label.into(), frame);
            // Place the stationary pointer on original fixture geometry BEFORE showing overlay.
            let first = &fixture.points[0];
            unsafe {
                let _ = ::windows::Win32::UI::WindowsAndMessaging::SetPhysicalCursorPos(
                    first.x, first.y,
                );
            }
            let window = WebviewWindowBuilder::new(
                app,
                label,
                WebviewUrl::App("index.html?view=capture".into()),
            )
            .title("Starshot original native acceptance overlay")
            .decorations(false)
            .resizable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .shadow(false)
            .visible(false)
            .data_directory(profile.path().join("webview"))
            .on_page_load(move |window, _| {
                let _ = window.eval(&script);
            })
            .build()?;
            window.set_position(tauri::PhysicalPosition::new(screen.x, screen.y))?;
            window.set_size(tauri::PhysicalSize::new(screen.width, screen.height))?;
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(35));
                handle.exit(2);
            });
            // Keep the ephemeral WebView profile alive through the native event loop.
            app.manage(profile);
            Ok(())
        })
        .build(config)
        .map_err(|e| e.to_string())?;
    app.run(|_, _| {});
    Ok(())
}
