//! Manual, offline verification with downloaded release artifacts only:
//! STARSHOT_UPDATER_SMOKE_DIR="$PWD/artifacts/updater-smoke" cargo test --locked \
//!   --manifest-path src-tauri/Cargo.toml --features updater-smoke \
//!   --test macos-updater-smoke -- --ignored --nocapture
//! The directory must contain the public 0.3.0 Starshot.app, 0.3.1 latest.json,
//! and the signed .app.tar.gz named by that manifest. No app is launched.

#![cfg(all(target_os = "macos", feature = "updater-smoke"))]

use std::{fs, path::Path, process::Command};
use tauri::{
    test::{mock_builder, mock_context, noop_assets},
    utils::config::AppDirectoriesOverride,
};
use tauri_plugin_updater::UpdaterExt;

fn run(command: &mut Command) -> String {
    let output = command.output().expect("run macOS verification command");
    assert!(
        output.status.success(),
        "{command:?} failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("command returned UTF-8")
}

fn version(app: &Path) -> String {
    run(Command::new("/usr/libexec/PlistBuddy")
        .args(["-c", "Print :CFBundleShortVersionString"])
        .arg(app.join("Contents/Info.plist")))
    .trim()
    .to_owned()
}

fn verify_codesign(app: &Path) {
    run(Command::new("/usr/bin/codesign")
        .args(["--verify", "--deep", "--strict", "--verbose=2"])
        .arg(app));
}

#[test]
#[ignore = "requires public 0.3.0 app and signed 0.3.1 artifacts; changes only a temporary copy"]
fn signed_update_replaces_temporary_app_and_rejects_corruption() {
    let fixtures = fs::canonicalize(
        std::env::var_os("STARSHOT_UPDATER_SMOKE_DIR")
            .expect("set STARSHOT_UPDATER_SMOKE_DIR to the release artifacts directory"),
    )
    .expect("release artifacts directory exists");
    let artifacts = fs::canonicalize(Path::new(env!("CARGO_MANIFEST_DIR")).join("../artifacts"))
        .expect("repository artifacts directory exists");
    assert!(
        fixtures.starts_with(artifacts),
        "fixtures must be in artifacts/"
    );
    let original = fixtures.join("Starshot.app");
    assert_eq!(version(&original), "0.3.0");
    verify_codesign(&original);

    let scratch = tempfile::Builder::new()
        .prefix("macos-updater-smoke-")
        .tempdir_in(&fixtures)
        .expect("create isolated test directory");
    let installed = scratch.path().join("Starshot.app");
    run(Command::new("/usr/bin/ditto")
        .arg(&original)
        .arg(&installed));

    let mut context = mock_context(noop_assets());
    context.package_info_mut().version = "0.3.0".parse().unwrap();
    context.config_mut().identifier = "com.starshot.updater-smoke".into();
    context.config_mut().app.app_directories_override = Some(AppDirectoriesOverride::Root(
        scratch.path().join("app-data"),
    ));
    let config: serde_json::Value =
        serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
    context
        .config_mut()
        .plugins
        .0
        .insert("updater".into(), config["plugins"]["updater"].clone());
    let app = mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)
        .expect("build windowless mock app");
    let updater = app
        .updater_builder()
        .executable_path(installed.join("Contents/MacOS/starshot"))
        .build()
        .expect("build updater for the temporary app only");
    let manifest =
        serde_json::from_slice(&fs::read(fixtures.join("latest.json")).unwrap()).unwrap();
    let update = updater
        .restore_update(manifest)
        .expect("restore the real release manifest")
        .expect("0.3.1 is newer than 0.3.0");
    assert_eq!(update.current_version, "0.3.0");
    assert_eq!(update.version, "0.3.1");
    let archive_name = update
        .download_url
        .path_segments()
        .unwrap()
        .next_back()
        .unwrap();
    assert!(archive_name.ends_with(".app.tar.gz"));
    let bytes = fs::read(fixtures.join(archive_name)).expect("read the real signed update archive");
    update
        .verify(&bytes)
        .expect("release signature and signed version are valid");

    let mut corrupted = bytes.clone();
    let middle = corrupted.len() / 2;
    corrupted[middle] ^= 1;
    let error = update
        .install(&corrupted)
        .expect_err("corruption must prevent installation");
    assert!(
        matches!(error, tauri_plugin_updater::Error::Minisign(_)),
        "{error}"
    );
    run(Command::new("/usr/bin/diff")
        .args(["-qr"])
        .arg(&original)
        .arg(&installed));
    assert_eq!(version(&installed), "0.3.0");
    verify_codesign(&installed);

    update
        .install(&bytes)
        .expect("install the verified release into the temporary app");
    assert_eq!(version(&installed), "0.3.1");
    assert!(installed.join("Contents/MacOS/starshot").is_file());
    verify_codesign(&installed);
    assert_eq!(version(&original), "0.3.0");
    println!("Verified: corrupted update rejected without changing 0.3.0; signed 0.3.1 installed and codesign passed. No app launched or restarted.");
}
