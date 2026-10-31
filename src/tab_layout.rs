//! Versioned workspace layout. Local paths use raw bytes, never lossy UTF-8.

use crate::config::StartupTabs;
use crate::tabs::TabName;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedLayout {
    pub version: u32,
    pub tabs: Vec<SavedTab>,
    pub selected: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedTab {
    #[serde(with = "path_bytes")]
    pub cwd: PathBuf,
    pub name: TabName,
}

mod path_bytes {
    use super::*;
    pub fn serialize<S: serde::Serializer>(path: &Path, serializer: S) -> Result<S::Ok, S::Error> {
        path.as_os_str().as_bytes().serialize(serializer)
    }
    pub fn deserialize<'de, D: serde::Deserializer<'de>>(d: D) -> Result<PathBuf, D::Error> {
        let bytes = Vec::<u8>::deserialize(d)?;
        if bytes.contains(&0) {
            return Err(serde::de::Error::custom("path contains NUL"));
        }
        Ok(std::ffi::OsString::from_vec(bytes).into())
    }
}

pub fn load(path: &Path) -> Result<Option<SavedLayout>, String> {
    let data = match std::fs::read(path) {
        Ok(data) => data,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(format!("could not read {}: {e}", path.display())),
    };
    decode(&data, path).map(Some)
}

fn decode(data: &[u8], path: &Path) -> Result<SavedLayout, String> {
    let saved: SavedLayout =
        serde_json::from_slice(data).map_err(|e| format!("invalid {}: {e}", path.display()))?;
    if saved.version != 1 {
        return Err(format!("unsupported tab layout version {}", saved.version));
    }
    Ok(saved)
}

pub struct LoadedLayout {
    pub saved: Option<SavedLayout>,
    pub notices: Vec<String>,
}

pub fn load_compatible(paths: &crate::app_paths::AppPaths) -> Result<LoadedLayout, String> {
    let Some(file) = crate::app_paths::read_preferred(&paths.layout, &paths.legacy_layout)? else {
        return Ok(LoadedLayout {
            saved: None,
            notices: vec![],
        });
    };
    let saved = Some(decode(&file.bytes, &file.path)?);
    let notices = if file.legacy {
        vec![format!(
            "Restoring legacy layout {}; future saves use {}",
            file.path.display(),
            paths.layout.display()
        )]
    } else {
        vec![]
    };
    Ok(LoadedLayout { saved, notices })
}

pub fn save_atomic(path: &Path, layout: &SavedLayout) -> Result<(), String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let parent = path.parent().ok_or("layout path has no parent")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temp = parent.join(format!(
        ".tabs-{}-{}.tmp",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|e| e.to_string())?;
    let result = (|| {
        file.write_all(&serde_json::to_vec(layout).map_err(std::io::Error::other)?)?;
        file.sync_all()?;
        std::fs::rename(&temp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.map_err(|e| format!("could not save {}: {e}", path.display()))
}

pub struct StartupLayout {
    pub tabs: Vec<SavedTab>,
    pub selected: usize,
    pub notices: Vec<String>,
}

pub fn snapshot(
    tabs: &crate::tabs::Tabs,
    observations: &[(crate::tabs::TabId, crate::process::SessionObservation)],
) -> SavedLayout {
    SavedLayout {
        version: 1,
        selected: tabs
            .ordered()
            .iter()
            .position(|tab| Some(tab.id) == tabs.active())
            .unwrap_or(0),
        tabs: tabs
            .ordered()
            .iter()
            .map(|tab| SavedTab {
                cwd: observations
                    .iter()
                    .find(|(id, _)| *id == tab.id)
                    .and_then(|(_, observation)| observation.cwd.clone())
                    .unwrap_or_else(|| tab.cwd.clone()),
                name: tab.name.clone(),
            })
            .collect(),
    }
}

pub fn resolve_startup(
    mode: StartupTabs,
    saved: Option<SavedLayout>,
    home: &Path,
) -> StartupLayout {
    let mut result = match (mode, saved) {
        (StartupTabs::Restore, Some(saved)) if !saved.tabs.is_empty() => StartupLayout {
            tabs: saved.tabs,
            selected: saved.selected,
            notices: vec![],
        },
        _ => StartupLayout {
            tabs: vec![SavedTab {
                cwd: home.into(),
                name: TabName::default(),
            }],
            selected: 0,
            notices: vec![],
        },
    };
    for tab in &mut result.tabs {
        if !tab.cwd.is_absolute() || !tab.cwd.is_dir() || std::fs::read_dir(&tab.cwd).is_err() {
            result.notices.push(format!(
                "{} is unavailable; opening home instead",
                tab.cwd.display()
            ));
            tab.cwd = home.into();
        }
    }
    result.selected = result.selected.min(result.tabs.len() - 1);
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::ffi::OsStringExt;

    #[test]
    fn compatible_layout_does_not_roll_back_a_bad_canonical_file() {
        let home = temp();
        let paths = crate::app_paths::AppPaths::new(&home);
        let saved = SavedLayout {
            version: 1,
            selected: 0,
            tabs: vec![SavedTab {
                cwd: home.clone(),
                name: TabName {
                    manual: Some("legacy".into()),
                    automatic: None,
                },
            }],
        };
        save_atomic(&paths.legacy_layout, &saved).unwrap();
        let old = std::fs::read(&paths.legacy_layout).unwrap();
        let loaded = load_compatible(&paths).unwrap();
        assert_eq!(loaded.saved, Some(saved));
        assert!(loaded.notices.join(" ").contains("Rake"));
        std::fs::create_dir_all(paths.layout.parent().unwrap()).unwrap();
        std::fs::write(&paths.layout, "broken").unwrap();
        assert!(load_compatible(&paths).is_err());
        assert_eq!(std::fs::read(&paths.legacy_layout).unwrap(), old);
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn snapshot_preserves_manual_and_automatic_names() {
        let mut tabs = crate::tabs::Tabs::new();
        let id = tabs.reserve_id().unwrap();
        tabs.insert(
            id,
            "/".into(),
            TabName {
                manual: Some("mine".into()),
                automatic: Some("ssh".into()),
            },
        );
        let layout = snapshot(
            &tabs,
            &[(
                id,
                crate::process::SessionObservation {
                    cwd: Some("/private".into()),
                    jobs: crate::process::JobState::Busy,
                },
            )],
        );
        assert_eq!(layout.tabs[0].cwd, PathBuf::from("/private"));
        assert_eq!(layout.tabs[0].name.manual.as_deref(), Some("mine"));
        assert_eq!(layout.tabs[0].name.automatic.as_deref(), Some("ssh"));
        assert_eq!(snapshot(&tabs, &[]).tabs[0].cwd, PathBuf::from("/"));
    }

    fn temp() -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "rake-layout-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn layout_round_trip_preserves_order_and_naming_mode() {
        let dir = temp();
        let path = dir.join("tabs.json");
        let layout = SavedLayout {
            version: 1,
            selected: 1,
            tabs: vec![
                SavedTab {
                    cwd: "/".into(),
                    name: TabName {
                        manual: Some("Build server".into()),
                        automatic: Some("ssh".into()),
                    },
                },
                SavedTab {
                    cwd: dir.clone(),
                    name: TabName::default(),
                },
            ],
        };
        save_atomic(&path, &layout).unwrap();
        assert_eq!(load(&path).unwrap(), Some(layout));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn non_utf8_cwd_round_trips() {
        let dir = temp();
        let layout = SavedLayout {
            version: 1,
            selected: 0,
            tabs: vec![SavedTab {
                cwd: std::ffi::OsString::from_vec(vec![b'/', 255]).into(),
                name: TabName::default(),
            }],
        };
        save_atomic(&dir.join("tabs.json"), &layout).unwrap();
        assert_eq!(load(&dir.join("tabs.json")).unwrap(), Some(layout));
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn invalid_or_newer_schema_reports_error() {
        let dir = temp();
        let path = dir.join("tabs.json");
        assert_eq!(load(&path).unwrap(), None);
        for content in ["{", r#"{"version":2,"tabs":[],"selected":0}"#] {
            std::fs::write(&path, content).unwrap();
            assert!(load(&path).is_err());
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn new_mode_ignores_saved_layout() {
        let home = temp();
        let saved = SavedLayout {
            version: 1,
            selected: 0,
            tabs: vec![SavedTab {
                cwd: "/".into(),
                name: TabName::default(),
            }],
        };
        let result = resolve_startup(StartupTabs::New, Some(saved), &home);
        assert_eq!(result.tabs.len(), 1);
        assert_eq!(result.tabs[0].cwd, home);
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn empty_or_missing_restore_opens_home() {
        for saved in [
            None,
            Some(SavedLayout {
                version: 1,
                selected: 99,
                tabs: vec![],
            }),
        ] {
            let result = resolve_startup(StartupTabs::Restore, saved, Path::new("/"));
            assert_eq!(result.tabs.len(), 1);
            assert_eq!(result.tabs[0].cwd, PathBuf::from("/"));
            assert_eq!(result.selected, 0);
        }
    }

    #[test]
    fn missing_cwd_falls_back_with_notice() {
        let home = temp();
        let saved = SavedLayout {
            version: 1,
            selected: 99,
            tabs: vec![SavedTab {
                cwd: home.join("missing"),
                name: TabName::default(),
            }],
        };
        let result = resolve_startup(StartupTabs::Restore, Some(saved), &home);
        assert_eq!(result.tabs[0].cwd, home);
        assert_eq!(result.notices.len(), 1);
        assert_eq!(result.selected, 0, "out of range selection clamps");
        std::fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn failed_replace_keeps_previous_layout() {
        let dir = temp();
        let target = dir.join("existing-directory");
        std::fs::create_dir(&target).unwrap();
        std::fs::write(target.join("keep"), "original").unwrap();
        assert!(save_atomic(
            &target,
            &SavedLayout {
                version: 1,
                tabs: vec![],
                selected: 0
            }
        )
        .is_err());
        assert_eq!(
            std::fs::read_to_string(target.join("keep")).unwrap(),
            "original"
        );
        assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
