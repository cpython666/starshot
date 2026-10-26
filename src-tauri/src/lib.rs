#[cfg(feature = "desktop")]
mod desktop;
pub mod imaging;
pub mod recovery;
#[cfg(feature = "desktop")]
mod updater;
#[cfg(feature = "desktop")]
pub use desktop::run;

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
            ("plugin:updater|check", [false, false, false]),
            ("plugin:updater|download", [false, false, false]),
            ("plugin:updater|install", [false, false, false]),
            ("plugin:updater|download_and_install", [false, false, false]),
            ("get_capture_frame", [false, true, false]),
            ("finish_capture", [false, true, false]),
            ("cancel_capture", [true, true, false]),
            ("get_pin_image", [false, false, true]),
            ("pin_action", [false, false, true]),
            ("plugin:dialog|save", [false, false, false]),
        ] {
            for (label, expected) in ["main", "capture-0", "pin-0"].into_iter().zip(permitted) {
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
