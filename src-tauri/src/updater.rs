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
    download_ready: bool,
    confirmation_pending: bool,
    total: Option<u64>,
    error: Option<String>,
    backup_id: Option<String>,
}

struct Pending {
    status: UpdateStatus,
    update: Option<Update>,
    downloaded_bytes: Option<Arc<Vec<u8>>>,
    cleanup_started: Arc<AtomicBool>,
}
pub struct Updates(Mutex<Pending>);

impl Pending {
    fn new() -> Self {
        Self {
            status: UpdateStatus {
                revision: 0,
                current_version: env!("CARGO_PKG_VERSION").into(),
                configured: true,
                phase: "idle",
                available: None,
                downloaded: 0,
                download_ready: false,
                confirmation_pending: false,
                total: None,
                error: None,
                backup_id: None,
            },
            update: None,
            downloaded_bytes: None,
            cleanup_started: Arc::new(AtomicBool::new(false)),
        }
    }
    fn begin_check(&mut self) -> Result<()> {
        ensure_idle(&self.status)?;
        self.update = None;
        self.downloaded_bytes = None;
        self.cleanup_started = Arc::new(AtomicBool::new(false));
        self.status.available = None;
        self.status.phase = "checking";
        self.status.error = None;
        self.status.downloaded = 0;
        self.status.download_ready = false;
        self.status.confirmation_pending = false;
        self.status.total = None;
        self.status.backup_id = None;
        Ok(())
    }
    fn validate_candidate(&self, expected: &str) -> Result<()> {
        let candidate = self.status.available.as_ref().ok_or("请先检查更新。")?;
        validate_expected_version(&candidate.version, expected)
    }
    fn begin_download(&mut self, expected: &str) -> Result<()> {
        ensure_idle(&self.status)?;
        self.validate_candidate(expected)?;
        if self.downloaded_bytes.is_some() {
            return Err("更新已下载，请选择安装并重启。".into());
        }
        self.status.phase = "downloading";
        self.status.error = None;
        self.status.downloaded = 0;
        self.status.download_ready = false;
        self.status.total = None;
        self.status.backup_id = None;
        Ok(())
    }
    fn downloaded(&mut self, expected: &str, bytes: Vec<u8>) -> Result<()> {
        if self.status.phase != "downloading" {
            return Err("下载状态已变化，请重新检查更新。".into());
        }
        self.validate_candidate(expected)?;
        if bytes.is_empty() {
            return Err("下载的更新包为空，无法安装。".into());
        }
        self.status.downloaded = bytes.len() as u64;
        self.status.total = Some(bytes.len() as u64);
        self.downloaded_bytes = Some(Arc::new(bytes));
        self.status.download_ready = true;
        self.status.confirmation_pending = true;
        self.status.phase = "ready";
        self.status.error = None;
        Ok(())
    }
    fn set_confirmation(&mut self, expected: &str, pending: bool) -> Result<()> {
        if !matches!(self.status.phase, "ready" | "error") {
            return Err("当前更新阶段不能更改安装确认。".into());
        }
        self.validate_candidate(expected)?;
        if !self.status.download_ready || self.downloaded_bytes.is_none() {
            return Err("请先下载并验证更新包。".into());
        }
        self.status.confirmation_pending = pending;
        Ok(())
    }
    fn begin_install(&mut self, expected: &str) -> Result<Arc<Vec<u8>>> {
        ensure_operation_idle(&self.status)?;
        self.validate_candidate(expected)?;
        let bytes = self
            .downloaded_bytes
            .clone()
            .ok_or("请先下载并验证更新包。")?;
        if !self.status.download_ready {
            return Err("请先下载并验证更新包。".into());
        }
        if !self.status.confirmation_pending {
            return Err("请先打开并确认安装更新。".into());
        }
        self.status.phase = "backing-up";
        self.status.error = None;
        self.status.backup_id = None;
        Ok(bytes)
    }
    fn installed(&mut self) {
        self.downloaded_bytes = None;
        self.status.download_ready = false;
        self.status.confirmation_pending = false;
        self.status.phase = "installed";
    }
    fn failed(&mut self, error: String) {
        // Only fully verified downloads enter the cache. A failed backup/install can
        // retry these same bytes; a failed download never becomes installable.
        self.status.download_ready = self.downloaded_bytes.is_some();
        self.status.phase = "error";
        self.status.error = Some(error);
    }
}

impl Updates {
    pub fn new() -> Self {
        Self(Mutex::new(Pending::new()))
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
    pub fn blocks_exit(&self) -> bool {
        self.0
            .lock()
            .map(|pending| {
                pending.status.confirmation_pending
                    || matches!(pending.status.phase, "backing-up" | "installing")
            })
            .unwrap_or(true)
    }
    pub fn begin_check(&self, app: &AppHandle) -> Result<()> {
        self.change(app, Pending::begin_check)
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
    pub fn begin_download(&self, app: &AppHandle, expected: &str) -> Result<Update> {
        self.change(app, |pending| {
            let update = pending.update.clone().ok_or("请先检查更新。")?;
            pending.begin_download(expected)?;
            Ok(update)
        })
    }
    pub fn downloaded(&self, app: &AppHandle, expected: &str, bytes: Vec<u8>) -> Result<()> {
        self.change(app, |pending| pending.downloaded(expected, bytes))
    }
    pub fn set_confirmation(&self, app: &AppHandle, expected: &str, pending: bool) -> Result<()> {
        self.change(app, |state| state.set_confirmation(expected, pending))
    }
    pub fn begin_install(
        &self,
        app: &AppHandle,
        expected: &str,
    ) -> Result<(Update, Arc<Vec<u8>>, Arc<AtomicBool>)> {
        self.change(app, |pending| {
            let update = pending.update.clone().ok_or("请先检查更新。")?;
            let bytes = pending.begin_install(expected)?;
            Ok((update, bytes, pending.cleanup_started.clone()))
        })
    }
    pub fn installed(&self, app: &AppHandle) -> Result<()> {
        self.change(app, |pending| {
            pending.installed();
            Ok(())
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
            pending.failed(error);
            Ok(())
        })?;
        self.status()
    }
}

fn ensure_idle(status: &UpdateStatus) -> Result<()> {
    ensure_operation_idle(status)?;
    if status.confirmation_pending {
        Err("请先确认安装更新或选择稍后安装。".into())
    } else {
        Ok(())
    }
}
fn ensure_operation_idle(status: &UpdateStatus) -> Result<()> {
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
    fn candidate() -> Pending {
        let mut pending = Pending::new();
        pending.status.available = Some(AvailableUpdate {
            version: "0.3.0".into(),
            date: None,
            notes: None,
        });
        pending.status.phase = "available";
        pending
    }

    #[test]
    fn download_must_finish_before_install_and_failure_cannot_become_ready() {
        let mut pending = candidate();
        assert!(pending.begin_install("0.3.0").is_err());
        assert_eq!(pending.status.phase, "available");
        assert!(pending.begin_download("0.4.0").is_err());
        pending.begin_download("0.3.0").unwrap();
        assert_eq!(pending.status.phase, "downloading");
        assert!(!pending.status.download_ready);
        assert!(pending.begin_install("0.3.0").is_err());
        assert!(pending.begin_download("0.3.0").is_err());
        pending.failed("signature verification failed".into());
        assert_eq!(pending.status.phase, "error");
        assert!(!pending.status.download_ready);
        assert!(!pending.status.confirmation_pending);
        assert!(pending.downloaded_bytes.is_none());
        assert!(pending.begin_install("0.3.0").is_err());
        assert!(pending.set_confirmation("0.3.0", true).is_err());
        pending.begin_download("0.3.0").unwrap();
        assert!(pending.downloaded("0.3.0", Vec::new()).is_err());
        pending.failed("empty download".into());
        assert!(!pending.status.download_ready);
    }

    #[test]
    fn verified_download_waits_for_confirmation_and_survives_backup_failure() {
        let mut pending = candidate();
        pending.begin_download("0.3.0").unwrap();
        pending.downloaded("0.3.0", vec![1, 2, 3]).unwrap();
        assert_eq!(pending.status.phase, "ready");
        assert!(pending.status.download_ready);
        assert_eq!(pending.status.downloaded, 3);
        assert!(pending.status.backup_id.is_none());
        assert!(!pending
            .cleanup_started
            .load(std::sync::atomic::Ordering::SeqCst));
        assert!(pending.status.confirmation_pending);
        assert!(ensure_idle(&pending.status).is_err());
        assert!(pending.begin_download("0.3.0").is_err());
        assert!(pending.begin_install("0.4.0").is_err());
        assert_eq!(pending.status.phase, "ready");
        let first = pending.begin_install("0.3.0").unwrap();
        assert_eq!(pending.status.phase, "backing-up");
        pending.failed("backup disk unavailable".into());
        assert!(pending.status.download_ready);
        assert!(pending.status.confirmation_pending);
        assert!(ensure_idle(&pending.status).is_err());
        pending.set_confirmation("0.3.0", false).unwrap();
        assert!(ensure_idle(&pending.status).is_ok());
        pending.set_confirmation("0.3.0", true).unwrap();
        let retry = pending.begin_install("0.3.0").unwrap();
        assert!(Arc::ptr_eq(&first, &retry));
        assert_eq!(retry.as_slice(), &[1, 2, 3]);
        pending.installed();
        assert_eq!(pending.status.phase, "installed");
        assert!(!pending.status.download_ready);
        assert!(!pending.status.confirmation_pending);
        assert!(pending.downloaded_bytes.is_none());
    }

    #[test]
    fn a_new_check_and_new_process_discard_ready_bytes_and_stale_confirmations() {
        let mut pending = candidate();
        pending.begin_download("0.3.0").unwrap();
        pending.downloaded("0.3.0", vec![1, 2, 3]).unwrap();
        assert!(pending.begin_check().is_err());
        assert!(pending.status.download_ready);
        pending.set_confirmation("0.3.0", false).unwrap();
        pending.begin_check().unwrap();
        assert_eq!(pending.status.phase, "checking");
        assert!(!pending.status.download_ready);
        assert!(!pending.status.confirmation_pending);
        assert!(pending.downloaded_bytes.is_none());
        assert!(pending.status.available.is_none());
        pending.status.available = Some(AvailableUpdate {
            version: "0.4.0".into(),
            date: None,
            notes: None,
        });
        pending.status.phase = "available";
        assert!(pending.begin_install("0.3.0").is_err());
        assert!(pending.begin_install("0.4.0").is_err());
        let fresh_process = Pending::new();
        assert!(!fresh_process.status.download_ready);
        assert!(!fresh_process.status.confirmation_pending);
        assert!(fresh_process.downloaded_bytes.is_none());
    }

    #[test]
    fn confirmation_gates_native_operations_until_deferred_or_installed() {
        let updates = Updates(Mutex::new(candidate()));
        {
            let mut pending = updates.0.lock().unwrap();
            assert!(pending.set_confirmation("0.3.0", true).is_err());
            pending.begin_download("0.3.0").unwrap();
            pending.downloaded("0.3.0", vec![1, 2, 3]).unwrap();
            assert!(pending.set_confirmation("0.4.0", false).is_err());
            assert!(pending.status.confirmation_pending);
        }
        assert!(updates.ensure_idle().is_err());
        assert!(updates.blocks_exit());
        {
            let mut pending = updates.0.lock().unwrap();
            pending.set_confirmation("0.3.0", false).unwrap();
            assert!(pending.begin_install("0.3.0").is_err());
            assert_eq!(pending.status.phase, "ready");
        }
        assert!(updates.ensure_idle().is_ok());
        assert!(!updates.blocks_exit());
        updates
            .0
            .lock()
            .unwrap()
            .set_confirmation("0.3.0", true)
            .unwrap();
        assert!(updates.ensure_idle().is_err());
        assert!(updates.blocks_exit());
        {
            let mut pending = updates.0.lock().unwrap();
            pending.begin_install("0.3.0").unwrap();
            assert!(pending.set_confirmation("0.3.0", false).is_err());
            assert!(pending.status.confirmation_pending);
            pending.status.phase = "installing";
            assert!(pending.set_confirmation("0.3.0", false).is_err());
            pending.installed();
        }
        assert!(!updates.blocks_exit());
        assert!(updates.allow_restart().is_ok());
    }

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
        for phase in ["idle", "error", "available", "up-to-date", "ready"] {
            status.phase = phase;
            assert!(ensure_idle(&status).is_ok());
            updates.0.lock().unwrap().status.phase = phase;
            assert!(updates.allow_restart().is_err());
        }
    }
}
