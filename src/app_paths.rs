//! Product paths and read-only compatibility selection. Runtime locks deliberately keep the old namespace.
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub config: PathBuf,
    pub legacy_config: PathBuf,
    pub layout: PathBuf,
    pub legacy_layout: PathBuf,
    pub runtime: PathBuf,
}

impl AppPaths {
    pub fn new(home: &Path) -> Self {
        let runtime = home.join("Library/Application Support/Rake");
        Self {
            config: home.join(".config/regie/config.toml"),
            legacy_config: home.join(".config/rake/config.toml"),
            layout: home.join("Library/Application Support/Regie/tabs.json"),
            legacy_layout: runtime.join("tabs.json"),
            runtime,
        }
    }
}

#[derive(Debug)]
pub struct LoadedFile {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
    pub legacy: bool,
}

fn read_optional(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) => {
            // A dangling symlink is a present, broken entry, not permission to roll back.
            if error.kind() == ErrorKind::NotFound
                && matches!(std::fs::symlink_metadata(path), Err(e) if e.kind() == ErrorKind::NotFound)
            {
                return Ok(None);
            }
            Err(format!("could not read {}: {error}", path.display()))
        }
    }
}

pub fn read_preferred(primary: &Path, legacy: &Path) -> Result<Option<LoadedFile>, String> {
    for (path, is_legacy) in [(primary, false), (legacy, true)] {
        if let Some(bytes) = read_optional(path)? {
            return Ok(Some(LoadedFile {
                path: path.into(),
                bytes,
                legacy: is_legacy,
            }));
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "regie-paths-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn regie_paths_retain_the_legacy_runtime_namespace() {
        let p = AppPaths::new(Path::new("/Users/test"));
        assert_eq!(
            p.config,
            PathBuf::from("/Users/test/.config/regie/config.toml")
        );
        assert_eq!(
            p.legacy_config,
            PathBuf::from("/Users/test/.config/rake/config.toml")
        );
        assert_eq!(
            p.layout,
            PathBuf::from("/Users/test/Library/Application Support/Regie/tabs.json")
        );
        assert_eq!(
            p.legacy_layout,
            PathBuf::from("/Users/test/Library/Application Support/Rake/tabs.json")
        );
        assert_eq!(
            p.runtime,
            PathBuf::from("/Users/test/Library/Application Support/Rake")
        );
    }

    #[test]
    fn preferred_read_uses_legacy_only_for_genuine_absence() {
        let t = Temp::new();
        let primary = t.0.join("primary");
        let legacy = t.0.join("legacy");
        assert!(read_preferred(&primary, &legacy).unwrap().is_none());
        std::fs::write(&legacy, b"old").unwrap();
        let read = read_preferred(&primary, &legacy).unwrap().unwrap();
        assert_eq!(read.bytes, b"old");
        assert!(read.legacy);
        assert_eq!(read.path, legacy);
        std::fs::write(&primary, b"new").unwrap();
        let read = read_preferred(&primary, &legacy).unwrap().unwrap();
        assert_eq!(read.bytes, b"new");
        assert!(!read.legacy);
        assert_eq!(read.path, primary);
        assert_eq!(std::fs::read(&legacy).unwrap(), b"old");
    }

    #[test]
    fn present_directory_or_broken_link_never_falls_back() {
        let t = Temp::new();
        let primary = t.0.join("primary");
        let legacy = t.0.join("legacy");
        std::fs::write(&legacy, b"old").unwrap();
        std::fs::create_dir(&primary).unwrap();
        assert!(read_preferred(&primary, &legacy)
            .unwrap_err()
            .contains("primary"));
        std::fs::remove_dir(&primary).unwrap();
        std::os::unix::fs::symlink(t.0.join("missing"), &primary).unwrap();
        assert!(read_preferred(&primary, &legacy)
            .unwrap_err()
            .contains("primary"));
    }

    #[test]
    fn denied_canonical_read_does_not_use_legacy() {
        use std::os::unix::fs::PermissionsExt;
        let t = Temp::new();
        let primary = t.0.join("primary");
        let legacy = t.0.join("legacy");
        std::fs::write(&primary, b"new").unwrap();
        std::fs::write(&legacy, b"old").unwrap();
        std::fs::set_permissions(&primary, std::fs::Permissions::from_mode(0o0)).unwrap();
        let result = read_preferred(&primary, &legacy);
        std::fs::set_permissions(&primary, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(result.unwrap_err().contains("primary"));
    }
}
