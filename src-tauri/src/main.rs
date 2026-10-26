#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
fn main() {
    if starshot_lib::ocr::run_worker_if_requested() {
        return;
    }
    starshot_lib::run();
}
