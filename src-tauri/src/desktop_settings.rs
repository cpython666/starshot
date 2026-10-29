use crate::imaging;
use serde::{Deserialize, Serialize};
use std::{fs, path::Path};
use tauri_plugin_global_shortcut::Shortcut;

type Result<T> = std::result::Result<T, String>;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettings {
    pub capture_shortcut: String,
    pub pin_shortcut: String,
    #[serde(default)]
    pub start_in_tray: bool,
    #[serde(default = "default_pin_halo")]
    pub pin_halo: bool,
}

fn default_pin_halo() -> bool {
    true
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopSettingsStatus {
    #[serde(flatten)]
    pub preferences: DesktopSettings,
    pub autostart: bool,
}

impl Default for DesktopSettings {
    fn default() -> Self {
        Self {
            capture_shortcut: "CommandOrControl+Shift+2".into(),
            pin_shortcut: "CommandOrControl+Alt+3".into(),
            start_in_tray: false,
            pin_halo: true,
        }
    }
}

impl DesktopSettings {
    pub fn shortcuts(&self) -> Result<[Shortcut; 2]> {
        if self.capture_shortcut.len() > 100 || self.pin_shortcut.len() > 100 {
            return Err("快捷键过长，请重新输入".into());
        }
        let capture = self
            .capture_shortcut
            .parse::<Shortcut>()
            .map_err(|_| "截图快捷键无效")?;
        let pin = self
            .pin_shortcut
            .parse::<Shortcut>()
            .map_err(|_| "贴图快捷键无效")?;
        if capture == pin {
            return Err("截图与贴图不能使用同一个快捷键".into());
        }
        Ok([capture, pin])
    }
}

pub fn load(directory: &Path) -> Result<DesktopSettings> {
    let path = directory.join("desktop-settings.json");
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(DesktopSettings::default())
        }
        Err(error) => return Err(error.to_string()),
    };
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > 4096 {
        return Err("桌面设置文件无效，已使用默认快捷键".into());
    }
    let settings: DesktopSettings =
        serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|_| "桌面设置无法读取，已使用默认快捷键")?;
    settings.shortcuts()?;
    Ok(settings)
}

pub fn save(directory: &Path, settings: &DesktopSettings) -> Result<()> {
    settings.shortcuts()?;
    fs::create_dir_all(directory).map_err(|e| e.to_string())?;
    let path = directory.join("desktop-settings.json");
    if fs::symlink_metadata(&path)
        .is_ok_and(|meta| !meta.is_file() || meta.file_type().is_symlink())
    {
        return Err("桌面设置文件不是普通文件，未写入".into());
    }
    imaging::atomic_save(
        &path,
        &serde_json::to_vec(settings).map_err(|e| e.to_string())?,
    )
}

// Only our two bindings are replaced; unrelated application registrations stay intact.
pub fn replace_shortcuts(
    old: &DesktopSettings,
    next: &DesktopSettings,
    mut binding: impl FnMut(Shortcut, bool) -> Result<bool>,
    persist: impl FnOnce() -> Result<()>,
) -> Result<()> {
    let previous = old.shortcuts()?;
    let replacement = next.shortcuts()?;
    let mut removed = Vec::new();
    let mut added = Vec::new();
    let result = (|| {
        for shortcut in previous {
            // Some platform APIs can change their bookkeeping before reporting
            // an unregister failure, so also roll back the attempted binding.
            removed.push(shortcut);
            if !binding(shortcut, false)? {
                removed.pop();
            }
        }
        for shortcut in replacement {
            binding(shortcut, true)?;
            added.push(shortcut);
        }
        persist()
    })();
    if let Err(error) = result {
        let mut rollback = Vec::new();
        for shortcut in added {
            if let Err(cause) = binding(shortcut, false) {
                rollback.push(cause);
            }
        }
        for shortcut in removed {
            if let Err(cause) = binding(shortcut, true) {
                rollback.push(cause);
            }
        }
        return Err(if rollback.is_empty() {
            format!("快捷键未更改，已恢复原设置：{error}")
        } else {
            format!(
                "快捷键更改失败：{error}。部分旧快捷键恢复失败，请使用按钮并重新设置：{}",
                rollback.join("；")
            )
        });
    }
    Ok(())
}

pub fn update_startup(
    previous: bool,
    next: bool,
    mut apply_and_check: impl FnMut(bool) -> Result<()>,
    persist: impl FnOnce() -> Result<()>,
) -> Result<()> {
    if previous == next {
        return persist();
    }
    let result = apply_and_check(next).and_then(|_| persist());
    if let Err(error) = result {
        return Err(match apply_and_check(previous) {
            Ok(()) => format!("启动设置未更改，已恢复原状态：{error}"),
            Err(rollback) => format!(
                "启动设置保存失败：{error}；恢复原系统状态失败：{rollback}。请刷新检查实际状态。"
            ),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn shortcut_changes_roll_back_registration_and_disk_failures() {
        let old = DesktopSettings::default();
        let next = DesktopSettings {
            capture_shortcut: "Control+Shift+4".into(),
            pin_shortcut: "Control+Shift+5".into(),
            start_in_tray: false,
            pin_halo: true,
        };
        let previous: HashSet<_> = old
            .shortcuts()
            .unwrap()
            .into_iter()
            .map(|s| s.id())
            .collect();
        for fail_registration in [true, false] {
            let mut active = previous.clone();
            let bad = next.shortcuts().unwrap()[1].id();
            let result = replace_shortcuts(
                &old,
                &next,
                |shortcut, register| {
                    if register && fail_registration && shortcut.id() == bad {
                        return Err("occupied".into());
                    }
                    if register {
                        active.insert(shortcut.id());
                    } else {
                        active.remove(&shortcut.id());
                    }
                    Ok(true)
                },
                || Err("disk full".into()),
            );
            assert!(result.is_err());
            assert_eq!(active, previous);
        }
        let duplicate = DesktopSettings {
            capture_shortcut: "Control+Shift+2".into(),
            pin_shortcut: "Ctrl+Shift+2".into(),
            start_in_tray: false,
            pin_halo: true,
        };
        assert!(duplicate.shortcuts().is_err());
    }
    #[test]
    fn failed_removal_restores_already_removed_binding_and_valid_settings_roundtrip() {
        let old = DesktopSettings::default();
        let previous = old.shortcuts().unwrap();
        let mut active: HashSet<_> = previous.into_iter().map(|shortcut| shortcut.id()).collect();
        let next = DesktopSettings {
            capture_shortcut: "Control+Shift+4".into(),
            ..old.clone()
        };
        let result = replace_shortcuts(
            &old,
            &next,
            |shortcut, register| {
                if !register && shortcut == previous[1] {
                    active.remove(&shortcut.id());
                    return Err("unregister failed".into());
                }
                if register {
                    active.insert(shortcut.id());
                } else {
                    active.remove(&shortcut.id());
                }
                Ok(true)
            },
            || Ok(()),
        );
        assert!(result.is_err());
        assert_eq!(
            active,
            previous.into_iter().map(|shortcut| shortcut.id()).collect()
        );
        let directory = tempfile::tempdir().unwrap();
        save(directory.path(), &old).unwrap();
        assert_eq!(load(directory.path()).unwrap(), old);
        fs::write(directory.path().join("desktop-settings.json"), "bad json").unwrap();
        assert!(load(directory.path()).is_err());
    }
    #[test]
    fn startup_failure_rolls_back_system_state_and_tray_preference_is_compatible() {
        let mut state = false;
        assert!(update_startup(
            false,
            true,
            |enabled| {
                state = enabled;
                Ok(())
            },
            || Err("disk full".into())
        )
        .is_err());
        assert!(!state);
        let parsed: DesktopSettings = serde_json::from_str(
            r#"{"captureShortcut":"Control+Shift+2","pinShortcut":"Control+Shift+3"}"#,
        )
        .unwrap();
        assert!(!parsed.start_in_tray);
        assert!(parsed.pin_halo);
        let json = serde_json::to_value(DesktopSettingsStatus {
            preferences: parsed,
            autostart: true,
        })
        .unwrap();
        assert_eq!(json["autostart"], true);
        assert_eq!(json["startInTray"], false);
        assert_eq!(json["pinHalo"], true);
    }
    #[test]
    fn pin_halo_defaults_on_and_disabled_preference_survives_reload() {
        let directory = tempfile::tempdir().unwrap();
        assert!(load(directory.path()).unwrap().pin_halo);
        let disabled = DesktopSettings {
            pin_halo: false,
            ..DesktopSettings::default()
        };
        save(directory.path(), &disabled).unwrap();
        assert_eq!(load(directory.path()).unwrap(), disabled);
        fs::write(
            directory.path().join("desktop-settings.json"),
            r#"{"captureShortcut":"Control+Shift+2","pinShortcut":"Control+Shift+3"}"#,
        )
        .unwrap();
        assert!(load(directory.path()).unwrap().pin_halo);
    }
    #[test]
    fn missing_old_binding_does_not_prevent_change_or_get_registered_during_rollback() {
        let old = DesktopSettings::default();
        let old_keys = old.shortcuts().unwrap();
        let next = DesktopSettings {
            capture_shortcut: "Control+Shift+4".into(),
            pin_shortcut: "Control+Shift+5".into(),
            start_in_tray: false,
            pin_halo: true,
        };
        for fail in [false, true] {
            let mut active = HashSet::from([old_keys[0].id()]);
            let target = next.shortcuts().unwrap();
            let result = replace_shortcuts(
                &old,
                &next,
                |key, register| {
                    if register {
                        if fail && key == target[1] {
                            return Err("occupied".into());
                        }
                        assert_ne!(key, old_keys[1]);
                        Ok(active.insert(key.id()))
                    } else {
                        Ok(active.remove(&key.id()))
                    }
                },
                || Ok(()),
            );
            if fail {
                assert!(result.is_err());
                assert_eq!(active, HashSet::from([old_keys[0].id()]));
            } else {
                assert!(result.is_ok());
                assert_eq!(active, target.into_iter().map(|key| key.id()).collect());
            }
        }
    }
}
