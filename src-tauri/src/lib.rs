#[cfg(feature = "desktop")]
mod desktop;
#[cfg(feature = "desktop")]
mod desktop_settings;
pub mod imaging;
pub mod ocr;
pub mod recovery;
pub mod scrolling;
#[cfg(feature = "desktop")]
mod updater;
#[cfg(any(feature = "desktop", test))]
mod window_regions;
#[cfg(feature = "desktop")]
pub use desktop::run;

pub fn run_uia_worker_if_requested() -> bool {
    #[cfg(all(feature = "desktop", target_os = "windows"))]
    {
        return window_regions::windows::run_worker_if_requested();
    }
    #[cfg(not(all(feature = "desktop", target_os = "windows")))]
    {
        false
    }
}

pub fn run_windows_smoke_if_requested() -> bool {
    #[cfg(all(feature = "windows-uia-smoke", target_os = "windows"))]
    {
        return desktop::windows_integration::run_if_requested()
            || desktop::windows_smoke::run_if_requested();
    }
    #[cfg(not(all(feature = "windows-uia-smoke", target_os = "windows")))]
    {
        false
    }
}

#[cfg(all(test, feature = "desktop"))]
mod capability_tests {
    #[test]
    fn runtime_acl_isolates_editor_capture_and_pin_commands() {
        let manifests = serde_json::from_str(include_str!(concat!(
            env!("OUT_DIR"),
            "/acl-manifests.json"
        )))
        .unwrap();
        let capabilities =
            serde_json::from_str(include_str!(concat!(env!("OUT_DIR"), "/capabilities.json")))
                .unwrap();
        let config = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let capabilities =
            tauri::utils::acl::get_capabilities(&config, capabilities, None).unwrap();
        let resolved = tauri::utils::acl::resolved::Resolved::resolve(
            &manifests,
            capabilities,
            tauri::utils::platform::Target::current(),
        )
        .unwrap();
        assert!(resolved.has_app_acl);
        let authority = tauri::runtime_authority!(manifests, resolved);
        for (command, permitted) in [
            ("start_capture", [true, false, false]),
            ("get_current_image", [true, false, false]),
            ("take_pending_pin_edit", [true, false, false]),
            ("finish_pin_edit", [true, false, false]),
            ("save_image", [true, false, false]),
            ("copy_image", [true, false, false]),
            ("get_update_status", [true, false, false]),
            ("check_starshot_update", [true, false, false]),
            ("download_starshot_update", [true, false, false]),
            ("set_update_confirmation", [true, false, false]),
            ("install_starshot_update", [true, false, false]),
            ("list_recovery_points", [true, false, false]),
            ("create_recovery_point", [true, false, false]),
            ("read_recovery_point", [true, false, false]),
            ("restart_app", [true, false, false]),
            ("get_desktop_settings", [true, false, false]),
            ("set_desktop_settings", [true, false, false]),
            ("set_editor_state", [true, false, false]),
            ("request_exit", [true, false, false]),
            ("cancel_exit", [true, false, false]),
            ("confirm_exit", [true, false, false]),
            ("release_empty_editor", [true, false, false]),
            ("cancel_idle_release", [true, false, false]),
            ("get_recovery_storage", [true, false, false]),
            ("delete_recovery_point", [true, false, false]),
            ("copy_text", [true, false, false]),
            ("reveal_saved_image", [true, false, false]),
            ("recognize_text", [true, false, false]),
            ("plugin:autostart|enable", [false, false, false]),
            ("plugin:autostart|disable", [false, false, false]),
            ("plugin:autostart|is_enabled", [false, false, false]),
            ("plugin:updater|check", [false, false, false]),
            ("plugin:updater|download", [false, false, false]),
            ("plugin:updater|install", [false, false, false]),
            ("plugin:updater|download_and_install", [false, false, false]),
            ("get_capture_frame", [false, true, false]),
            ("capture_overlay_ready", [false, true, false]),
            ("get_capture_element", [false, true, false]),
            ("invalidate_capture_element", [false, true, false]),
            ("native_smoke_report", [false, true, false]),
            ("native_smoke_move", [false, true, false]),
            ("finish_capture", [false, true, false]),
            ("begin_scroll_capture", [false, true, false]),
            ("append_scroll_capture", [false, true, false]),
            ("finish_scroll_capture", [false, true, false]),
            ("cancel_capture", [true, true, false]),
            ("get_pin_image", [false, false, true]),
            ("pin_action", [false, false, true]),
            ("pin_export", [false, false, true]),
            ("plugin:dialog|save", [false, false, false]),
        ] {
            for (label, expected) in ["main", "capture-42-0", "pin-0"].into_iter().zip(permitted) {
                assert_eq!(
                    authority
                        .resolve_access(command, label, label, &tauri::ipc::Origin::Local)
                        .is_some(),
                    expected,
                    "{command} on {label}"
                );
            }
        }
    }
}
