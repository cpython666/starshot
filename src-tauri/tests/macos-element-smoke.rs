//! Manual native AX verification against our original AppKit fixture only.
//! xcrun swiftc tests/macos-panel-fixture.swift -o artifacts/macos-panel-fixture
//! STARSHOT_AX_FIXTURE="$PWD/artifacts/macos-panel-fixture" cargo test --locked \
//!   --manifest-path src-tauri/Cargo.toml --test macos-element-smoke \
//!   original_panels_form_nested_selection_paths -- --ignored --nocapture
//! Requires existing Accessibility permission for the test process. This test
//! neither requests permission nor captures pixels or reads other applications.

#![cfg(all(feature = "desktop", target_os = "macos"))]

mod imaging {
    pub use starshot_lib::imaging::*;
}
#[allow(dead_code, unused_imports)]
#[path = "../src/window_regions.rs"]
mod window_regions;

use serde::Deserialize;
use std::{
    io::{BufRead, BufReader, Read},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicU64, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};
use window_regions::{ElementBounds, WindowBounds};

struct Fixture(Child);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[derive(Deserialize)]
struct Point {
    x: f64,
    y: f64,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    point: Point,
    expected: Vec<ElementBounds>,
}

#[derive(Deserialize)]
struct Ready {
    pid: u32,
    window: ElementBounds,
    cases: Vec<Case>,
}

fn start_fixture() -> (Fixture, Ready) {
    let executable = std::env::var_os("STARSHOT_AX_FIXTURE").expect(
        "set STARSHOT_AX_FIXTURE to the compiled tests/macos-panel-fixture.swift executable",
    );
    let mut fixture = Fixture(
        Command::new(executable)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("start original AppKit fixture"),
    );
    let stdout = fixture.0.stdout.take().expect("fixture stdout pipe");
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let mut line = String::new();
        let result = BufReader::new(stdout.take(65_536))
            .read_line(&mut line)
            .map(|_| line);
        let _ = tx.send(result);
    });
    let line = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("fixture must publish its JSON readiness line within 10 seconds")
        .expect("read fixture readiness line");
    let ready: Ready = serde_json::from_str(&line).expect("parse fixture geometry JSON");
    assert_eq!(ready.pid, fixture.0.id(), "only query the spawned fixture");
    (fixture, ready)
}

fn same(a: ElementBounds, b: ElementBounds) -> bool {
    (a.x - b.x).abs() < 0.01
        && (a.y - b.y).abs() < 0.01
        && (a.width - b.width).abs() < 0.01
        && (a.height - b.height).abs() < 0.01
}

fn encloses(a: ElementBounds, b: ElementBounds) -> bool {
    a.x <= b.x + 0.01
        && a.y <= b.y + 0.01
        && a.x + a.width + 0.01 >= b.x + b.width
        && a.y + a.height + 0.01 >= b.y + b.height
}

#[test]
#[ignore = "launches only the original AppKit fixture; needs existing Accessibility permission"]
fn original_panels_form_nested_selection_paths() {
    assert!(
        window_regions::element_detection_available(),
        "Accessibility permission is unavailable to this test process; no permission was requested or changed"
    );
    let (_fixture, ready) = start_fixture();
    let mut names: Vec<_> = ready.cases.iter().map(|case| case.name.as_str()).collect();
    names.sort_unstable();
    assert_eq!(
        names,
        [
            "editor-body",
            "editor-button",
            "sidebar-button",
            "toolbar-button",
            "zero-frame-parent",
        ],
        "all original fixture cases must be present"
    );
    let host = WindowBounds {
        pid: ready.pid,
        x: ready.window.x.floor() as i32,
        y: ready.window.y.floor() as i32,
        width: ((ready.window.x + ready.window.width).ceil() - ready.window.x.floor()) as u32,
        height: ((ready.window.y + ready.window.height).ceil() - ready.window.y.floor()) as u32,
        native: None,
    };
    let revision = AtomicU64::new(1);
    for case in &ready.cases {
        let point = (case.point.x, case.point.y);
        let actual = window_regions::elements_at(host, point, &revision, 1);
        assert!(
            !case.expected.is_empty(),
            "{} must specify a real nested path",
            case.name
        );
        let mut next = 0;
        for expected in &case.expected {
            let offset = actual[next..]
                .iter()
                .position(|candidate| same(*candidate, *expected))
                .unwrap_or_else(|| {
                    panic!(
                        "{}: missing ordered ancestor {expected:?}; actual={actual:?}",
                        case.name
                    )
                });
            next += offset + 1;
        }
        for (index, rect) in actual.iter().enumerate() {
            assert!(
                [rect.x, rect.y, rect.width, rect.height]
                    .into_iter()
                    .all(f64::is_finite)
                    && rect.width >= 2.0
                    && rect.height >= 2.0
                    && rect.x <= point.0
                    && point.0 < rect.x + rect.width
                    && rect.y <= point.1
                    && point.1 < rect.y + rect.height
                    && encloses(ready.window, *rect)
                    && !same(ready.window, *rect),
                "{}: invalid extra ancestor {rect:?}",
                case.name
            );
            if index > 0 {
                assert!(
                    encloses(*rect, actual[index - 1]) && !same(*rect, actual[index - 1]),
                    "{}: selection path must expand without duplicate bounds",
                    case.name
                );
            }
        }
        println!("{}: {} native ancestors verified", case.name, actual.len());
    }

    revision.store(2, Ordering::Release);
    let point = (ready.cases[0].point.x, ready.cases[0].point.y);
    assert!(window_regions::elements_at(host, point, &revision, 1).is_empty());
    let outside = (f64::from(host.x) - 1.0, f64::from(host.y) + 1.0);
    assert!(window_regions::elements_at(host, outside, &revision, 2).is_empty());
    println!("cancelled revision and point outside fixture host: empty results verified");
}
