#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    // A normal binary must not turn an obsolete acceptance command into a daily app launch.
    #[cfg(not(feature = "windows-uia-smoke"))]
    if std::env::args().nth(1).is_some_and(|arg| {
        matches!(
            arg.as_str(),
            "--starshot-uia-probe" | "--starshot-uia-overlay" | "--starshot-uia-integration"
        )
    }) {
        std::process::exit(64);
    }
    if starshot_lib::run_uia_worker_if_requested() {
        return;
    }
    if starshot_lib::run_windows_smoke_if_requested() {
        return;
    }
    if starshot_lib::ocr::run_worker_if_requested() {
        return;
    }
    starshot_lib::run();
}
