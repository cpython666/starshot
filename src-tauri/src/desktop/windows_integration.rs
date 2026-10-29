//! Explicit acceptance mode: production initialization, generated pixels, isolated WebViews.
//! File commands contain only indices into predeclared original fixture geometry.
use super::{
    begin_capture, begin_exit, clear_capture, lock, Context, FramePayload, Region, Result, Updates,
};
use crate::window_regions::windows::{self, Request};
use image::DynamicImage;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager, WebviewWindow, WebviewWindowBuilder};

struct Integration {
    directory: PathBuf,
    fixtures: Vec<PathBuf>,
    records: Mutex<HashMap<String, (u64, Value)>>,
    pixel_calls: AtomicU64,
    calls: Mutex<HashMap<String, Vec<Value>>>,
    clock: Instant,
    moves: Mutex<Vec<Value>>,
}

fn cursor_info() -> Value {
    use ::windows::Win32::{
        Foundation::POINT,
        UI::{HiDpi::*, WindowsAndMessaging::*},
    };
    unsafe {
        let mut logical = POINT::default();
        let mut physical = POINT::default();
        let logical_ok = GetCursorPos(&mut logical).is_ok();
        let physical_ok = GetPhysicalCursorPos(&mut physical).is_ok();
        json!({"cursor":{"x":logical.x,"y":logical.y},"physical":{"x":physical.x,"y":physical.y},"logicalOk":logical_ok,"physicalOk":physical_ok,
            "threadPmv2":AreDpiAwarenessContextsEqual(GetThreadDpiAwarenessContext(),DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).as_bool()})
    }
}

fn record_call(app: &AppHandle, label: &str, command: &str, args: Value) -> usize {
    let state = app.state::<Integration>();
    let mut calls = state.calls.lock().unwrap();
    let rows = calls.entry(label.into()).or_default();
    rows.push(json!({"command":command,"args":args,"start":state.clock.elapsed().as_micros() as f64/1000.0}));
    rows.len() - 1
}
fn complete_call(app: &AppHandle, label: &str, index: usize, result: Value) {
    let state = app.state::<Integration>();
    if let Some(row) = state
        .calls
        .lock()
        .unwrap()
        .get_mut(label)
        .and_then(|rows| rows.get_mut(index))
    {
        row["end"] = json!(state.clock.elapsed().as_micros() as f64 / 1000.0);
        row["result"] = result;
    };
}
#[tauri::command]
fn get_capture_frame(app: AppHandle, window: WebviewWindow) -> Result<FramePayload> {
    let index = record_call(&app, window.label(), "get_capture_frame", Value::Null);
    let result = super::get_capture_frame(app.clone(), window.clone());
    if let Ok(f) = &result {
        complete_call(
            &app,
            window.label(),
            index,
            json!({"width":f.width,"height":f.height,"pointer":f.pointer,"scaleFactor":f.scale_factor,"cursorInfo":cursor_info()}),
        );
    }
    result
}
#[tauri::command]
async fn get_capture_element(
    app: AppHandle,
    window: WebviewWindow,
    point: crate::window_regions::ImagePoint,
    generation: u64,
) -> Result<Vec<Region>> {
    let index = record_call(
        &app,
        window.label(),
        "get_capture_element",
        json!({"point":point,"generation":generation}),
    );
    // A user moving outside the declared original fixtures invalidates the sample;
    // never turn it into a UIA query of a personal application during acceptance.
    let allowed = {
        let state = app.state::<Integration>();
        let pids: Vec<_> = state
            .fixtures
            .iter()
            .filter_map(|p| std::fs::read(p).ok())
            .filter_map(|b| serde_json::from_slice::<Value>(&b).ok())
            .filter_map(|v| v["pid"].as_u64())
            .collect();
        let context = app.state::<Context>();
        let frames = lock(&context.frames)?;
        frames
            .get(window.label())
            .and_then(|f| {
                f.hosts.iter().find(|h| {
                    point.x >= h.rect.x
                        && point.y >= h.rect.y
                        && point.x < h.rect.x + h.rect.width
                        && point.y < h.rect.y + h.rect.height
                })
            })
            .is_some_and(|h| pids.contains(&u64::from(h.pid)))
    };
    let result = if allowed {
        super::get_capture_element(app.clone(), window.clone(), point, generation).await
    } else {
        Err("acceptance pointer outside original fixture".into())
    };
    complete_call(&app, window.label(), index, json!(result.as_ref().ok()));
    result
}
#[tauri::command]
fn invalidate_capture_element(
    app: AppHandle,
    window: WebviewWindow,
    generation: u64,
) -> Result<()> {
    let index = record_call(
        &app,
        window.label(),
        "invalidate_capture_element",
        json!({"generation":generation}),
    );
    let result = super::invalidate_capture_element(app.clone(), window.clone(), generation);
    complete_call(&app, window.label(), index, json!(result.is_ok()));
    result
}

pub(super) fn pixels(app: &AppHandle, width: u32, height: u32) -> Option<DynamicImage> {
    let state = app.try_state::<Integration>()?;
    state.pixel_calls.fetch_add(1, Ordering::Relaxed);
    Some(DynamicImage::ImageRgba8(image::RgbaImage::from_fn(
        width,
        height,
        |x, y| {
            image::Rgba(if (x / 80 + y / 80) % 2 == 0 {
                [26, 32, 46, 255]
            } else {
                [36, 44, 60, 255]
            })
        },
    )))
}

pub(super) fn configure<'a>(
    app: &AppHandle,
    builder: WebviewWindowBuilder<'a, tauri::Wry, AppHandle>,
) -> WebviewWindowBuilder<'a, tauri::Wry, AppHandle> {
    if let Some(state) = app.try_state::<Integration>() {
        builder
            .data_directory(state.directory.join("webview"))
            .initialization_script(include_str!("../../../tests/windows-integration-driver.js"))
    } else {
        builder
    }
}

#[tauri::command]
fn native_integration_record(
    app: AppHandle,
    window: WebviewWindow,
    sequence: u64,
    records: Value,
) -> Result<()> {
    if serde_json::to_vec(&records)
        .map_err(|e| e.to_string())?
        .len()
        > 1024 * 1024
    {
        return Err("acceptance record too large".into());
    }
    lock(&app.state::<Integration>().records)?.insert(window.label().into(), (sequence, records));
    Ok(())
}

fn fixture(app: &AppHandle, command: &Value) -> Result<(Value, Value)> {
    let state = app.state::<Integration>();
    let index = command["fixture"]
        .as_u64()
        .ok_or("fixture index required")? as usize;
    let path = state.fixtures.get(index).ok_or("unknown fixture")?;
    let data: Value = serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let point = data["points"]
        .as_array()
        .ok_or("points missing")?
        .iter()
        .find(|p| p["id"] == command["point"])
        .ok_or("unknown fixture point")?
        .clone();
    Ok((data, point))
}

fn move_pointer(app: &AppHandle, command: &Value) -> Result<()> {
    let (_, p) = fixture(app, command)?;
    unsafe {
        ::windows::Win32::UI::WindowsAndMessaging::SetPhysicalCursorPos(
            p["x"].as_i64().ok_or("x")? as i32,
            p["y"].as_i64().ok_or("y")? as i32,
        )
    }
    .map_err(|e| e.to_string())?;
    let state = app.state::<Integration>();
    lock(&state.moves)?.push(json!({"ms":state.clock.elapsed().as_millis(),"expected":{"x":p["x"],"y":p["y"]},"actual":cursor_info()}));
    Ok(())
}

fn frames(app: &AppHandle) -> Result<Value> {
    let context = app.state::<Context>();
    let state = app.state::<Integration>();
    let pids: Vec<_> = state
        .fixtures
        .iter()
        .filter_map(|p| std::fs::read(p).ok())
        .filter_map(|b| serde_json::from_slice::<Value>(&b).ok())
        .filter_map(|v| v["pid"].as_u64())
        .collect();
    let frames = lock(&context.frames)?;
    let mut rows: Vec<_> = frames.iter().map(|(label, f)| {
        json!({"label":label,"monitor":{"x":f.monitor.x,"y":f.monitor.y,"width":f.monitor.width,"height":f.monitor.height,"scale":f.monitor.scale},
            "generation":f.element_generation.load(Ordering::Acquire),"point":f.element_point,
            "hosts":f.hosts.iter().filter(|h|pids.contains(&u64::from(h.pid))).map(|h|json!({"window":h.window,"rect":h.rect})).collect::<Vec<_>>(),
            "ownHostCount":f.hosts.iter().filter(|h|h.pid==std::process::id()).count()})
    }).collect();
    drop(frames);
    // Native window getters dispatch to the UI thread, where IPC also uses frames.
    // Never retain either diagnostic/state mutex across that dispatch.
    for row in &mut rows {
        let window = app.get_webview_window(row["label"].as_str().unwrap());
        row["actualPosition"] = json!(window
            .as_ref()
            .and_then(|w| w.inner_position().ok())
            .map(|p| json!({"x":p.x,"y":p.y})));
        row["actualSize"] = json!(window
            .as_ref()
            .and_then(|w| w.inner_size().ok())
            .map(|p| json!({"width":p.width,"height":p.height})));
    }
    Ok(json!(rows))
}

fn observe(app: &AppHandle, sequence: u64) -> Result<Value> {
    let sizes: Vec<_> = lock(&app.state::<Context>().frames)?
        .iter()
        .map(|(l, f)| (l.clone(), f.payload.width, f.payload.height))
        .collect();
    let labels: Vec<_> = sizes.iter().map(|(l, _, _)| l.clone()).collect();
    for (label, width, height) in &sizes {
        app.get_webview_window(label)
            .ok_or("overlay missing")?
            .eval(format!(
                "window.__starshotSample?.({sequence},{width},{height})"
            ))
            .map_err(|e| e.to_string())?;
    }
    let start = Instant::now();
    loop {
        let state = app.state::<Integration>();
        let records = lock(&state.records)?;
        if labels
            .iter()
            .all(|l| records.get(l).is_some_and(|r| r.0 == sequence))
        {
            let mut samples: Vec<_> = labels
                .iter()
                .map(|l| json!({"label":l,"view":records[l].1}))
                .collect();
            drop(records);
            let calls = state.calls.lock().unwrap();
            for sample in &mut samples {
                let rows = calls
                    .get(sample["label"].as_str().unwrap())
                    .cloned()
                    .unwrap_or_default();
                sample["view"]["frame"] = rows
                    .iter()
                    .find(|r| r["command"] == "get_capture_frame")
                    .map(|r| r["result"].clone())
                    .unwrap_or(Value::Null);
                sample["view"]["calls"] = json!(rows);
            }
            drop(calls);
            return Ok(json!({"views":samples,"frames":frames(app)?}));
        }
        if start.elapsed() > Duration::from_secs(4) {
            return Err("WebView sample deadline".into());
        }
        drop(records);
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn command(app: &AppHandle, value: &Value, sequence: u64) -> Result<Value> {
    match value["action"].as_str().ok_or("action required")? {
        "start" => {
            move_pointer(app, value)?;
            begin_capture(app)?;
            frames(app)
        }
        "move" => {
            move_pointer(app, value)?;
            Ok(json!(true))
        }
        "sweep" => {
            let final_index = value["fixture"]
                .as_u64()
                .filter(|n| *n < 2)
                .ok_or("monitor fixture")?;
            let mut moves = Vec::new();
            let started = Instant::now();
            for index in [1 - final_index, final_index, 1 - final_index, final_index] {
                move_pointer(app, &json!({"fixture":index,"point":"edit"}))?;
                moves.push(
                    json!({"fixture":index,"ms":started.elapsed().as_micros() as f64/1000.0}),
                );
                std::thread::sleep(Duration::from_millis(110));
            }
            Ok(json!(moves))
        }
        "reap-delay" => {
            let ms = value["ms"]
                .as_u64()
                .filter(|ms| *ms == 0 || *ms == 150)
                .ok_or("invalid diagnostic delay")?;
            windows::REAP_DELAY_MS.store(ms, Ordering::Relaxed);
            Ok(json!(ms))
        }
        "raise" => {
            for window in app.webview_windows().values() {
                window.set_always_on_top(false).map_err(|e| e.to_string())?;
                window.set_always_on_top(true).map_err(|e| e.to_string())?;
            }
            Ok(json!(true))
        }
        "observe" => observe(app, sequence),
        "clear" => {
            clear_capture(app)?;
            lock(&app.state::<Integration>().records)?.clear();
            lock(&app.state::<Integration>().calls)?.clear();
            Ok(json!(true))
        }
        "query" => {
            let (f, p) = fixture(app, value)?;
            let target = windows::describe(f["hwnd"].as_i64().ok_or("hwnd")? as isize, 0)
                .ok_or("fixture unavailable")?;
            if Some(u64::from(target.pid)) != f["pid"].as_u64() {
                return Err("fixture identity mismatch".into());
            }
            let before = super::windows_smoke::census();
            let generation = Arc::new(AtomicU64::new(sequence));
            let cancel = value["cancelMs"]
                .as_u64()
                .filter(|ms| *ms <= 1000)
                .map(|ms| {
                    let generation = generation.clone();
                    std::thread::spawn(move || {
                        std::thread::sleep(Duration::from_millis(ms));
                        generation.fetch_add(1, Ordering::Release);
                    })
                });
            let started = Instant::now();
            let reply = windows::query_reply(
                Request {
                    target,
                    x: p["x"].as_i64().ok_or("x")? as i32,
                    y: p["y"].as_i64().ok_or("y")? as i32,
                },
                &generation,
                sequence,
            );
            if let Some(c) = cancel {
                let _ = c.join();
            }
            Ok(
                json!({"expected":p["expected"],"reply":reply,"wallMs":started.elapsed().as_millis(),"before":before,"after":super::windows_smoke::census()}),
            )
        }
        "finish" => {
            clear_capture(app)?;
            Ok(json!(true))
        }
        _ => Err("unknown acceptance action".into()),
    }
}

pub(crate) fn run_if_requested() -> bool {
    let args: Vec<_> = std::env::args().collect();
    if args.get(1).map(String::as_str) != Some("--starshot-uia-integration") {
        return false;
    }
    let result = (|| -> Result<()> {
        let path = PathBuf::from(args.get(2).ok_or("acceptance config required")?);
        let config: Value =
            serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let directory = path.parent().ok_or("config directory")?.to_path_buf();
        let fixtures: Vec<PathBuf> =
            serde_json::from_value(config["fixtures"].clone()).map_err(|e| e.to_string())?;
        if fixtures.len() != 3 {
            return Err("three original fixtures required".into());
        }
        let mut context = tauri::generate_context!();
        context.config_mut().app.windows.clear();
        let state = Context::default();
        lock(&state.editor)?.released = true;
        let app = tauri::Builder::default().manage(state).manage(Updates::new())
            .manage(Integration { directory:directory.clone(), fixtures, records:Mutex::new(HashMap::new()), pixel_calls:AtomicU64::new(0), calls:Mutex::new(HashMap::new()), clock:Instant::now(), moves:Mutex::new(Vec::new()) })
            .invoke_handler(tauri::generate_handler![get_capture_frame,get_capture_element,invalidate_capture_element,super::cancel_capture,native_integration_record])
            .setup(move |app| {
                let handle=app.handle().clone();
                std::thread::spawn(move || {
                    let started=Instant::now(); let mut last=0;
                    let _=std::fs::write(directory.join("ready.json"), json!({"pid":std::process::id(),"census":super::windows_smoke::census()}).to_string());
                    while started.elapsed()<Duration::from_secs(1200) {
                        let value=std::fs::read(directory.join("command.json")).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok());
                        if let Some(value)=value {
                            if let Some(sequence)=value["sequence"].as_u64().filter(|n|*n>last) {
                                last=sequence; let start=Instant::now();
                                let result=command(&handle,&value,sequence);
                                let row=json!({"sequence":sequence,"parentPid":std::process::id(),"atMs":started.elapsed().as_millis(),"wallMs":start.elapsed().as_millis(),
                                    "result":result.as_ref().ok(),"error":result.as_ref().err(),"trace":windows::take_trace(),"census":super::windows_smoke::census(),
                                    "cursorInfo":cursor_info(),"moves":std::mem::take(&mut *handle.state::<Integration>().moves.lock().unwrap()),"generatedPixelCalls":handle.state::<Integration>().pixel_calls.load(Ordering::Relaxed)});
                                let output=directory.join(format!("response-{sequence}.json"));
                                let temp=output.with_extension("tmp");
                                let _=std::fs::write(&temp,row.to_string()); let _=std::fs::rename(temp,output);
                                if value["action"]=="finish" { let _=begin_exit(&handle); return; }
                            }
                        }
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    handle.exit(2);
                });
                Ok(())
            }).build(context).map_err(|e|e.to_string())?;
        app.run(|_, event| {
            if let tauri::RunEvent::ExitRequested {
                code: None, api, ..
            } = event
            {
                api.prevent_exit(); // Keep the same parent alive between capture sessions.
            }
        });
        Ok(())
    })();
    if let Err(e) = result {
        eprintln!("native integration: {e}");
        std::process::exit(1);
    }
    true
}
