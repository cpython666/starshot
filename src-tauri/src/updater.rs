type Result<T> = std::result::Result<T, String>;
use serde::Serialize;
use std::{
    fs,
    path::Path,
    sync::{atomic::AtomicBool, Arc, Mutex},
};
use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::Update;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    version: String,
    date: Option<String>,
    notes: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    revision: u64,
    current_version: String,
    configured: bool,
    phase: &'static str,
    available: Option<AvailableUpdate>,
    downloaded: u64,
    total: Option<u64>,
    error: Option<String>,
    backup_id: Option<String>,
}

struct Pending {
    status: UpdateStatus,
    update: Option<Update>,
    cleanup_started: Arc<AtomicBool>,
}
pub struct Updates(Mutex<Pending>);

impl Updates {
    pub fn new() -> Self {
        Self(Mutex::new(Pending {
            status: UpdateStatus {
                revision: 0,
                current_version: env!("CARGO_PKG_VERSION").into(),
                configured: true,
                phase: "idle",
                available: None,
                downloaded: 0,
                total: None,
                error: None,
                backup_id: None,
            },
            update: None,
            cleanup_started: Arc::new(AtomicBool::new(false)),
        }))
    }
    fn change<T>(
        &self,
        app: &AppHandle,
        action: impl FnOnce(&mut Pending) -> Result<T>,
    ) -> Result<T> {
        let mut pending = self.0.lock().map_err(|_| "更新状态异常，请重启星镜。")?;
        let result = action(&mut pending)?;
        pending.status.revision += 1;
        let _ = app.emit_to("main", "starshot-update-status", &pending.status);
        Ok(result)
    }
    pub fn status(&self) -> Result<UpdateStatus> {
        Ok(self
            .0
            .lock()
            .map_err(|_| "更新状态异常，请重启星镜。")?
            .status
            .clone())
    }
    pub fn ensure_idle(&self) -> Result<()> {
        ensure_idle(
            &self
                .0
                .lock()
                .map_err(|_| "更新状态异常，请重启星镜。")?
                .status,
        )
    }
    pub fn allow_restart(&self) -> Result<()> {
        let status = self.status()?;
        if matches!(status.phase, "installed" | "restart-required") {
            Ok(())
        } else {
            Err("请在更新完成后再重启星镜。".into())
        }
    }
    pub fn installing(&self) -> bool {
        self.0
            .lock()
            .map(|pending| matches!(pending.status.phase, "backing-up" | "installing"))
            .unwrap_or(true)
    }
    pub fn begin_check(&self, app: &AppHandle) -> Result<()> {
        self.change(app, |pending| {
            ensure_idle(&pending.status)?;
            pending.update = None;
            pending.cleanup_started = Arc::new(AtomicBool::new(false));
            pending.status.available = None;
            pending.status.phase = "checking";
            pending.status.error = None;
            pending.status.downloaded = 0;
            pending.status.total = None;
            pending.status.backup_id = None;
            Ok(())
        })
    }
    pub fn checked(&self, app: &AppHandle, mut update: Option<Update>) -> Result<()> {
        if let Some(update) = &mut update {
            validate_download_url(update.download_url.as_str())?;
            update.timeout = Some(std::time::Duration::from_secs(600));
        }
        self.change(app, |pending| {
            pending.status.available = update.as_ref().map(|update| AvailableUpdate {
                version: update.version.clone(),
                date: update
                    .raw_json
                    .get("pub_date")
                    .and_then(|date| date.as_str())
                    .map(str::to_owned),
                notes: update.body.clone(),
            });
            pending.status.phase = if update.is_some() {
                "available"
            } else {
                "up-to-date"
            };
            pending.update = update;
            Ok(())
        })
    }
    pub fn cleanup_flag(&self) -> Result<Arc<AtomicBool>> {
        Ok(self
            .0
            .lock()
            .map_err(|_| "更新状态异常，请重启星镜。")?
            .cleanup_started
            .clone())
    }
    pub fn begin_install(
        &self,
        app: &AppHandle,
        expected: &str,
    ) -> Result<(Update, Arc<AtomicBool>)> {
        self.change(app, |pending| {
            ensure_idle(&pending.status)?;
            let update = pending.update.as_ref().ok_or("请先检查更新。")?;
            validate_expected_version(&update.version, expected)?;
            pending.status.phase = "downloading";
            pending.status.error = None;
            pending.status.downloaded = 0;
            pending.status.total = None;
            pending.status.backup_id = None;
            Ok((update.clone(), pending.cleanup_started.clone()))
        })
    }
    pub fn progress(&self, app: &AppHandle, downloaded: u64, total: Option<u64>) {
        let _ = self.change(app, |pending| {
            pending.status.downloaded = downloaded;
            pending.status.total = total;
            Ok(())
        });
    }
    pub fn phase(&self, app: &AppHandle, phase: &'static str) -> Result<()> {
        self.change(app, |pending| {
            pending.status.phase = phase;
            Ok(())
        })
    }
    pub fn backed_up(&self, app: &AppHandle, id: String) -> Result<()> {
        self.change(app, |pending| {
            pending.status.backup_id = Some(id);
            Ok(())
        })
    }
    pub fn failed(&self, app: &AppHandle, error: String) -> Result<UpdateStatus> {
        self.change(app, |pending| {
            pending.status.phase = "error";
            pending.status.error = Some(error);
            Ok(())
        })?;
        self.status()
    }
    pub fn restart_required(&self, app: &AppHandle, error: String) -> Result<UpdateStatus> {
        self.failed(app, error)?;
        self.phase(app, "restart-required")?;
        self.status()
    }
}

fn ensure_idle(status: &UpdateStatus) -> Result<()> {
    if matches!(
        status.phase,
        "checking" | "downloading" | "backing-up" | "installing" | "installed" | "restart-required"
    ) {
        Err("更新操作正在进行或等待重启，请先完成当前操作。".into())
    } else {
        Ok(())
    }
}
fn validate_expected_version(actual: &str, expected: &str) -> Result<()> {
    if actual == expected {
        Ok(())
    } else {
        Err("可用版本已变化，请重新检查并确认更新。".into())
    }
}
fn validate_download_url(url: &str) -> Result<()> {
    // The signed bytes remain authoritative; restrict metadata's download origin too.
    if url.starts_with("https://github.com/cpython666/starshot/releases/download/") {
        Ok(())
    } else {
        Err("更新安装包地址不属于星镜正式发布源，已停止更新。".into())
    }
}

/// Preserve the executable separately from data snapshots. Never overwrite it.
pub fn preserve_application(default: &Path, backup_id: &str) -> Result<()> {
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    let directory = default.join("application-backups").join(backup_id);
    #[cfg(target_os = "macos")]
    let source = executable
        .ancestors()
        .nth(3)
        .filter(|path| path.extension().is_some_and(|ext| ext == "app"))
        .ok_or("请先将星镜安装到应用程序目录，再使用应用内更新。")?;
    fs::create_dir_all(directory.parent().ok_or("应用备份目录无效。")?)
        .map_err(|error| error.to_string())?;
    fs::create_dir(&directory).map_err(|error| format!("无法创建旧版应用备份：{error}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
            .map_err(|error| error.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("/usr/bin/ditto")
            .arg(source)
            .arg(directory.join("Starshot.app"))
            .output()
            .map_err(|error| format!("旧版应用备份失败，未开始安装：{error}"))?;
        if !output.status.success() {
            return Err("旧版应用备份失败，未开始安装。请检查磁盘空间与目录权限。".into());
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let target = directory.join(executable.file_name().ok_or("应用路径无效。")?);
        fs::copy(&executable, &target)
            .and_then(|_| fs::File::options().write(true).open(&target)?.sync_all())
            .map_err(|error| format!("旧版应用备份失败，未开始安装：{error}"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn update_validation_rejects_changed_confirmation_and_untrusted_origin() {
        assert!(validate_expected_version("0.3.0", "0.2.0").is_err());
        assert!(validate_expected_version("0.3.0", "0.3.0").is_ok());
        assert!(validate_download_url(
            "https://github.com/cpython666/starshot/releases/download/v0.3.0/app.tar.gz"
        )
        .is_ok());
        for url in [
            "http://github.com/cpython666/starshot/releases/download/a",
            "https://github.com.evil/cpython666/starshot/releases/download/a",
            "https://github.com/other/starshot/releases/download/a",
        ] {
            assert!(validate_download_url(url).is_err());
        }
        let updates = Updates::new();
        let mut status = updates.status().unwrap();
        for phase in [
            "checking",
            "downloading",
            "backing-up",
            "installing",
            "installed",
            "restart-required",
        ] {
            status.phase = phase;
            assert!(ensure_idle(&status).is_err());
            updates.0.lock().unwrap().status.phase = phase;
            assert_eq!(
                updates.allow_restart().is_ok(),
                matches!(phase, "installed" | "restart-required")
            );
        }
        for phase in ["idle", "error", "available", "up-to-date"] {
            status.phase = phase;
            assert!(ensure_idle(&status).is_ok());
            updates.0.lock().unwrap().status.phase = phase;
            assert!(updates.allow_restart().is_err());
        }
    }
}
