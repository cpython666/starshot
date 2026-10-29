//! Geometry-only Windows UIA. The only COM client lives in a disposable copy of
//! our executable. A Job Object and parent deadline, not a COM Future, bound it.
use super::{ElementBounds, NativeWindow, WindowBounds};
use ::windows::{
    core::BOOL,
    Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE, HWND, LPARAM, RECT},
        Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED},
        System::{
            Com::{
                CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_INPROC_SERVER,
                COINIT_MULTITHREADED,
            },
            JobObjects::{
                AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
                SetInformationJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
                JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
            },
            Threading::{GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
        },
        UI::{
            Accessibility::{
                CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTreeWalker,
            },
            HiDpi::{
                AreDpiAwarenessContextsEqual, GetThreadDpiAwarenessContext,
                SetThreadDpiAwarenessContext, DPI_AWARENESS_CONTEXT,
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            },
            WindowsAndMessaging::{
                EnumWindows, GetClassLongW, GetWindowRect, GetWindowThreadProcessId, IsIconic,
                IsWindow, IsWindowVisible, GCW_ATOM,
            },
        },
    },
};
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    os::windows::{io::AsRawHandle, process::CommandExt},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};

const FLAG: &str = "--starshot-uia-worker";
const OUTPUT_LIMIT: u64 = 8192;
const TIMEOUT: Duration = Duration::from_millis(750);
const SLOT_WAIT: Duration = Duration::from_millis(250);
const POLL_INTERVAL: Duration = Duration::from_millis(5);
static SINGLE_QUERY: Mutex<()> = Mutex::new(());

#[cfg(feature = "windows-uia-smoke")]
static TRACE: Mutex<Vec<serde_json::Value>> = Mutex::new(Vec::new());
// Controlled cleanup-latency fault, acceptance feature only; ordinary runs use zero.
#[cfg(feature = "windows-uia-smoke")]
pub(crate) static REAP_DELAY_MS: AtomicU64 = AtomicU64::new(0);
#[cfg(feature = "windows-uia-smoke")]
fn trace_ms() -> f64 {
    static START: std::sync::OnceLock<Instant> = std::sync::OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_micros() as f64 / 1000.0
}
#[cfg(feature = "windows-uia-smoke")]
pub(crate) fn trace_gate(label: &str, generation: u64, status: &str) {
    let mut rows = TRACE.lock().unwrap();
    if rows.len() < 20000 {
        rows.push(serde_json::json!({"event":"gate","ms":trace_ms(),"parentPid":std::process::id(),"label":label,"generation":generation,"status":status}));
    }
}
#[cfg(feature = "windows-uia-smoke")]
fn trace(event: &str, request: &Request, expected: u64, helper: Option<u32>, status: &str) {
    let mut rows = TRACE.lock().unwrap();
    if rows.len() < 20000 {
        rows.push(serde_json::json!({"event":event,"ms":trace_ms(),
            "parentPid":std::process::id(),"generation":expected,"helperPid":helper,"targetPid":request.target.pid,
            "hwnd":request.target.native.map(|n|n.hwnd),"x":request.x,"y":request.y,"status":status}));
    }
}
#[cfg(feature = "windows-uia-smoke")]
pub(crate) fn take_trace() -> Vec<serde_json::Value> {
    std::mem::take(&mut *TRACE.lock().unwrap())
}

struct Dpi(DPI_AWARENESS_CONTEXT);
impl Dpi {
    fn physical() -> Self {
        Self(unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) })
    }
}
impl Drop for Dpi {
    fn drop(&mut self) {
        if !self.0 .0.is_null() {
            unsafe {
                SetThreadDpiAwarenessContext(self.0);
            }
        }
    }
}
struct Handle(HANDLE);
impl Drop for Handle {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

fn hwnd(value: isize) -> HWND {
    HWND(value as *mut _)
}

pub fn describe(handle: isize, z_order: u32) -> Option<WindowBounds> {
    let _dpi = Dpi::physical();
    let h = hwnd(handle);
    unsafe {
        if !IsWindow(Some(h)).as_bool() || !IsWindowVisible(h).as_bool() || IsIconic(h).as_bool() {
            return None;
        }
        let mut pid = 0;
        let thread_id = GetWindowThreadProcessId(h, Some(&mut pid));
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid)
            .ok()
            .map(Handle);
        let (mut created, mut exited, mut kernel, mut user) = (
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
            FILETIME::default(),
        );
        let identity_available = process.as_ref().is_some_and(|p| {
            GetProcessTimes(p.0, &mut created, &mut exited, &mut kernel, &mut user).is_ok()
        });
        let mut r = RECT::default();
        GetWindowRect(h, &mut r).ok()?;
        let width = i64::from(r.right) - i64::from(r.left);
        let height = i64::from(r.bottom) - i64::from(r.top);
        if width <= 0 || height <= 0 {
            return None;
        }
        Some(WindowBounds {
            pid,
            x: r.left,
            y: r.top,
            width: width.try_into().ok()?,
            height: height.try_into().ok()?,
            // Even an inaccessible process can still contribute a window fallback.
            native: identity_available.then_some(NativeWindow {
                hwnd: handle,
                thread_id,
                process_started: (u64::from(created.dwHighDateTime) << 32)
                    | u64::from(created.dwLowDateTime),
                class_atom: GetClassLongW(h, GCW_ATOM) as u16,
                z_order,
            }),
        })
    }
}

pub fn valid(target: WindowBounds) -> bool {
    let _dpi = Dpi::physical();
    target
        .native
        .is_some_and(|n| describe(n.hwnd, n.z_order) == Some(target))
}

pub fn snapshot(own_pid: u32) -> Result<Vec<WindowBounds>, String> {
    let _dpi = Dpi::physical();
    struct Inventory {
        own: u32,
        order: u32,
        windows: Vec<WindowBounds>,
    }
    unsafe extern "system" fn visit(h: HWND, param: LPARAM) -> BOOL {
        let state = &mut *(param.0 as *mut Inventory);
        state.order += 1;
        if state.order > 1024 {
            return BOOL(0);
        }
        let mut pid = 0;
        GetWindowThreadProcessId(h, Some(&mut pid));
        if pid == state.own {
            return BOOL(1);
        }
        let mut cloaked = 0u32;
        let _ = DwmGetWindowAttribute(h, DWMWA_CLOAKED, (&mut cloaked as *mut u32).cast(), 4);
        if cloaked == 0 {
            if let Some(w) = describe(h.0 as isize, state.order) {
                state.windows.push(w);
            }
        }
        BOOL(1)
    }
    let mut state = Inventory {
        own: own_pid,
        order: 0,
        windows: Vec::new(),
    };
    unsafe {
        EnumWindows(Some(visit), LPARAM((&mut state as *mut Inventory) as isize))
            .map_err(|e| e.to_string())?;
    }
    Ok(state.windows)
}

#[derive(Serialize, Deserialize)]
pub struct Request {
    pub target: WindowBounds,
    pub x: i32,
    pub y: i32,
}
#[derive(Default, Serialize, Deserialize)]
pub struct Reply {
    pub status: String,
    pub candidates: Vec<ElementBounds>,
    pub visited: usize,
    pub elapsed_ms: u128,
    pub pmv2: bool,
    pub dpi_set: bool,
}

struct ManagedChild {
    child: Child,
    _job: Handle,
}
impl Drop for ManagedChild {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
impl ManagedChild {
    fn spawn() -> Option<Self> {
        let job = Handle(unsafe { CreateJobObjectW(None, None).ok()? });
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&limits as *const JOBOBJECT_EXTENDED_LIMIT_INFORMATION).cast(),
                std::mem::size_of_val(&limits) as u32,
            )
            .ok()?;
        }
        let child = Command::new(std::env::current_exe().ok()?)
            .arg(FLAG)
            .creation_flags(0x0800_0000)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let managed = Self { child, _job: job };
        unsafe {
            AssignProcessToJobObject(managed._job.0, HANDLE(managed.child.as_raw_handle())).ok()?;
        }
        Some(managed)
    }
}

/// At most one owned helper; no target process is ever killed.
/// Cancellation is checked without holding Context::operation or the frame lock.
pub fn query(request: Request, generation: &AtomicU64, expected: u64) -> Vec<ElementBounds> {
    query_reply(request, generation, expected).candidates
}
pub fn query_reply(request: Request, generation: &AtomicU64, expected: u64) -> Reply {
    #[cfg(feature = "windows-uia-smoke")]
    trace("request", &request, expected, None, "");
    let reply = query_reply_inner(&request, generation, expected);
    #[cfg(feature = "windows-uia-smoke")]
    trace("reply", &request, expected, None, &reply.status);
    reply
}
fn query_reply_inner(request: &Request, generation: &AtomicU64, expected: u64) -> Reply {
    // Another monitor may still be starting/reaping its cancelled helper. A
    // bounded retry lets a stationary pointer refine after that slot is released.
    let waiting = Instant::now();
    loop {
        let status = if generation.load(Ordering::Acquire) != expected {
            Some("cancelled")
        } else if waiting.elapsed() >= SLOT_WAIT {
            Some("busy")
        } else {
            None
        };
        if let Some(status) = status {
            return Reply {
                status: status.into(),
                elapsed_ms: waiting.elapsed().as_millis(),
                ..Reply::default()
            };
        }
        let reply = query_reply_once(request, generation, expected);
        if reply.status != "busy" {
            return reply;
        }
        std::thread::sleep(POLL_INTERVAL.min(SLOT_WAIT.saturating_sub(waiting.elapsed())));
    }
}
fn query_reply_once(request: &Request, generation: &AtomicU64, expected: u64) -> Reply {
    let fallback = |status: &str| Reply {
        status: status.into(),
        ..Reply::default()
    };
    let Ok(_slot) = SINGLE_QUERY.try_lock() else {
        return fallback("busy");
    };
    if generation.load(Ordering::Acquire) != expected {
        return fallback("cancelled");
    }
    if !valid(request.target) {
        return fallback("stale-target");
    }
    let Some(mut managed) = ManagedChild::spawn() else {
        return fallback("helper-unavailable");
    };
    #[cfg(feature = "windows-uia-smoke")]
    let helper_pid = managed.child.id();
    #[cfg(feature = "windows-uia-smoke")]
    trace("spawn", request, expected, Some(helper_pid), "");
    let started = Instant::now();
    let packet = match serde_json::to_vec(request) {
        Ok(v) if v.len() < 2048 => v,
        _ => return fallback("bad-request"),
    };
    let Some(mut input) = managed.child.stdin.take() else {
        return fallback("pipe");
    };
    if input.write_all(&packet).is_err() {
        return fallback("pipe");
    }
    drop(input);
    let Some(output) = managed.child.stdout.take() else {
        return fallback("pipe");
    };
    // Drain concurrently: even bounded JSON must not deadlock on a small pipe buffer.
    let reader = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        output
            .take(OUTPUT_LIMIT + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let outcome = loop {
        if generation.load(Ordering::Acquire) != expected {
            break "cancelled";
        }
        if started.elapsed() >= TIMEOUT {
            break "timeout";
        }
        match managed.child.try_wait() {
            Ok(Some(code)) => break if code.success() { "ok" } else { "helper-error" },
            Err(_) => break "helper-error",
            Ok(None) => std::thread::sleep(POLL_INTERVAL),
        }
    };
    // Kill/reap before joining the pipe reader. Job ownership also survives parent exit.
    #[cfg(feature = "windows-uia-smoke")]
    if outcome == "cancelled" {
        let delay = REAP_DELAY_MS.load(Ordering::Relaxed).min(150);
        if delay > 0 {
            trace(
                "delay",
                request,
                expected,
                Some(helper_pid),
                "injected-reap-150ms",
            );
            std::thread::sleep(Duration::from_millis(delay));
        }
    }
    drop(managed);
    #[cfg(feature = "windows-uia-smoke")]
    trace("reap", request, expected, Some(helper_pid), outcome);
    let bytes = reader
        .join()
        .ok()
        .and_then(Result::ok)
        .filter(|b| b.len() as u64 <= OUTPUT_LIMIT);
    if outcome != "ok" {
        return Reply {
            elapsed_ms: started.elapsed().as_millis(),
            ..fallback(outcome)
        };
    }
    if generation.load(Ordering::Acquire) != expected || !valid(request.target) {
        return fallback("stale-target");
    }
    bytes
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_else(|| fallback("bad-response"))
}

#[derive(Clone)]
struct Candidate {
    bounds: ElementBounds,
    control: i32,
    path: Vec<usize>,
}
fn contains(r: ElementBounds, x: i32, y: i32) -> bool {
    [r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite())
        && r.width >= 2.0
        && r.height >= 2.0
        && f64::from(x) >= r.x
        && f64::from(y) >= r.y
        && f64::from(x) < r.x + r.width
        && f64::from(y) < r.y + r.height
}
fn semantic(control: i32) -> bool {
    matches!(control, 50000 | 50002..=50005 | 50007..=50009 | 50011 | 50013 | 50015 | 50016 | 50018 | 50019 | 50029)
}
fn rank(c: &Candidate) -> (u8, u64, usize) {
    (
        if semantic(c.control) {
            0
        } else if c.control == 50020 {
            2
        } else {
            1
        },
        (c.bounds.width * c.bounds.height) as u64,
        usize::MAX - c.path.len(),
    )
}
fn related(a: &[usize], b: &[usize]) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

fn optional_element(
    result: ::windows::core::Result<IUIAutomationElement>,
) -> Result<Option<IUIAutomationElement>, ()> {
    match result {
        Ok(element) => Ok(Some(element)),
        // windows-rs represents a successful null COM output as Error::empty()
        // (HRESULT 0). A failed provider call must NOT masquerade as end-of-tree.
        Err(error) if error.code().is_ok() => Ok(None),
        Err(_) => Err(()),
    }
}

unsafe fn walk(
    element: &IUIAutomationElement,
    walker: &IUIAutomationTreeWalker,
    request: &Request,
    path: &mut Vec<usize>,
    parent: ElementBounds,
    candidates: &mut Vec<Candidate>,
    visited: &mut usize,
    started: Instant,
) -> Result<(), ()> {
    if *visited >= 128 || path.len() >= 12 || started.elapsed() > Duration::from_millis(300) {
        return Err(());
    }
    *visited += 1;
    if element.CurrentProcessId().map_err(|_| ())? != request.target.pid as i32 {
        return Ok(());
    }
    let r = element.CurrentBoundingRectangle().map_err(|_| ())?;
    let left = f64::from(r.left).max(parent.x);
    let top = f64::from(r.top).max(parent.y);
    let right = f64::from(r.right).min(parent.x + parent.width);
    let bottom = f64::from(r.bottom).min(parent.y + parent.height);
    let bounds = ElementBounds {
        x: left,
        y: top,
        width: right - left,
        height: bottom - top,
    };
    if !contains(bounds, request.x, request.y) {
        return Ok(());
    }
    let control = element.CurrentControlType().map_err(|_| ())?.0;
    if !path.is_empty() {
        candidates.push(Candidate {
            bounds,
            control,
            path: path.clone(),
        });
    }
    let mut child = optional_element(walker.GetFirstChildElement(element))?;
    let mut index = 0;
    while let Some(item) = child {
        path.push(index);
        walk(
            &item, walker, request, path, bounds, candidates, visited, started,
        )?;
        path.pop();
        child = optional_element(walker.GetNextSiblingElement(&item))?;
        index += 1;
    }
    Ok(())
}

unsafe fn inspect(request: &Request) -> Reply {
    let started = Instant::now();
    let _dpi = Dpi::physical();
    let mut reply = Reply {
        status: "unavailable".into(),
        dpi_set: !_dpi.0 .0.is_null(),
        pmv2: AreDpiAwarenessContextsEqual(
            GetThreadDpiAwarenessContext(),
            DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
        )
        .as_bool(),
        ..Reply::default()
    };
    if !reply.pmv2 || !valid(request.target) {
        reply.status = "stale-target".into();
        return reply;
    }
    if CoInitializeEx(None, COINIT_MULTITHREADED).is_err() {
        return reply;
    }
    struct Com;
    impl Drop for Com {
        fn drop(&mut self) {
            unsafe {
                CoUninitialize();
            }
        }
    }
    let _com = Com;
    let result = (|| -> Option<Vec<ElementBounds>> {
        let uia: IUIAutomation =
            CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()?;
        let root = uia
            .ElementFromHandle(hwnd(request.target.native?.hwnd))
            .ok()?;
        let walker = uia.RawViewWalker().ok()?;
        let host = ElementBounds {
            x: f64::from(request.target.x),
            y: f64::from(request.target.y),
            width: f64::from(request.target.width),
            height: f64::from(request.target.height),
        };
        let mut candidates = Vec::new();
        if walk(
            &root,
            &walker,
            request,
            &mut Vec::new(),
            host,
            &mut candidates,
            &mut reply.visited,
            started,
        )
        .is_err()
        {
            reply.status = "budget-or-provider-error".into();
            return None;
        }
        // Inspect ALL containing siblings. Unrelated overlapping branches have no
        // universal UIA z-order; refusing ambiguity is safer than selecting a hidden button.
        candidates.sort_by_key(rank);
        let best = candidates.first()?;
        if candidates.iter().any(|c| !related(&best.path, &c.path)) {
            reply.status = "ambiguous-overlap".into();
            return None;
        }
        let mut result = Vec::new();
        for c in &candidates {
            if semantic(best.control) && c.control == 50020 {
                continue;
            }
            if !result.iter().any(|r: &ElementBounds| {
                r.x == c.bounds.x
                    && r.y == c.bounds.y
                    && r.width == c.bounds.width
                    && r.height == c.bounds.height
            }) {
                result.push(c.bounds);
            }
            if result.len() == 8 {
                break;
            }
        }
        reply.status = "ok".into();
        Some(result)
    })();
    reply.candidates = result.unwrap_or_default();
    if !valid(request.target) {
        reply.status = "stale-target".into();
        reply.candidates.clear();
    }
    reply.elapsed_ms = started.elapsed().as_millis();
    reply
}

pub fn run_worker_if_requested() -> bool {
    if std::env::args().nth(1).as_deref() != Some(FLAG) {
        return false;
    }
    let result = (|| {
        let mut bytes = Vec::new();
        std::io::stdin().take(2049).read_to_end(&mut bytes).ok()?;
        if bytes.len() > 2048 {
            return None;
        }
        let request: Request = serde_json::from_slice(&bytes).ok()?;
        Some(unsafe { inspect(&request) })
    })()
    .unwrap_or_default();
    if let Ok(bytes) = serde_json::to_vec(&result) {
        if bytes.len() as u64 <= OUTPUT_LIMIT {
            let _ = std::io::stdout().write_all(&bytes);
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{mpsc, Arc};
    static QUERY_TEST: Mutex<()> = Mutex::new(());

    fn unavailable_target() -> Request {
        Request {
            target: WindowBounds {
                pid: 0,
                x: 0,
                y: 0,
                width: 10,
                height: 10,
                native: None,
            },
            x: 1,
            y: 1,
        }
    }

    // The handshake confirms the caller thread has started; a timed receive
    // then checks that contention did not immediately escape as an empty reply.
    fn contending_query(generation: Arc<AtomicU64>) -> mpsc::Receiver<Reply> {
        let (started_tx, started_rx) = mpsc::channel();
        let (reply_tx, reply_rx) = mpsc::channel();
        std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let reply = query_reply(unavailable_target(), &generation, 0);
            reply_tx.send(reply).unwrap();
        });
        started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(matches!(
            reply_rx.recv_timeout(Duration::from_millis(30)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ));
        reply_rx
    }

    #[test]
    fn contended_query_resumes_when_another_monitor_releases_the_slot() {
        let _test = QUERY_TEST.lock().unwrap();
        let slot = SINGLE_QUERY.lock().unwrap();
        let reply = contending_query(Arc::new(AtomicU64::new(0)));
        drop(slot);
        // An invalid identity stops before starting a helper or reading any UI.
        assert_eq!(
            reply.recv_timeout(Duration::from_secs(2)).unwrap().status,
            "stale-target"
        );
    }

    #[test]
    fn contended_query_cancels_without_waiting_for_the_slot() {
        let _test = QUERY_TEST.lock().unwrap();
        let _slot = SINGLE_QUERY.lock().unwrap();
        let generation = Arc::new(AtomicU64::new(0));
        let reply = contending_query(generation.clone());
        generation.store(1, Ordering::Release);
        // Keep the slot held: completion must come from cancellation, not release.
        assert_eq!(
            reply.recv_timeout(Duration::from_secs(2)).unwrap().status,
            "cancelled"
        );
    }

    #[test]
    fn contended_query_stops_at_the_slot_wait_budget() {
        let _test = QUERY_TEST.lock().unwrap();
        let _slot = SINGLE_QUERY.lock().unwrap();
        let started = Instant::now();
        let reply = query_reply(unavailable_target(), &AtomicU64::new(0), 0);
        assert_eq!(reply.status, "busy");
        assert!(started.elapsed() >= SLOT_WAIT);
        // OS scheduling can add overhead; this is not an exact wall-clock limit.
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn semantic_button_beats_its_smaller_label_and_overlap_is_not_ancestry() {
        let button = Candidate {
            bounds: ElementBounds {
                x: -200.0,
                y: 10.0,
                width: 180.0,
                height: 45.0,
            },
            control: 50000,
            path: vec![0, 1],
        };
        let label = Candidate {
            bounds: ElementBounds {
                x: -180.0,
                y: 20.0,
                width: 80.0,
                height: 15.0,
            },
            control: 50020,
            path: vec![0, 1, 0],
        };
        assert!(rank(&button) < rank(&label));
        assert!(related(&button.path, &label.path));
        assert!(!related(&button.path, &[0, 2]));
        assert!(contains(button.bounds, -200, 10));
        assert!(!contains(button.bounds, -20, 10)); // half-open right boundary
    }
}
