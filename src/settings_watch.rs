//! Observe the directories containing the canonical and legacy settings files.

use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::mpsc::{self, Receiver, RecvTimeoutError, Sender},
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::app_paths::AppPaths;

const COALESCE_WINDOW: Duration = Duration::from_millis(40);

/// Keeps filesystem notifications active while settings files are edited externally.
pub struct SettingsWatch {
    stop: Option<Sender<()>>,
    worker: Option<JoinHandle<()>>,
}

impl SettingsWatch {
    pub fn new(paths: AppPaths, wake: impl Fn() + Send + Sync + 'static) -> Result<Self, String> {
        let (events_tx, events_rx) = mpsc::channel();
        let mut watcher = notify::recommended_watcher(move |event| {
            let _ = events_tx.send(event);
        })
        .map_err(|error| format!("could not create settings watcher: {error}"))?;
        let watched = directories_to_watch(&paths);
        if watched.is_empty() {
            return Err("could not find an existing ancestor for settings files".into());
        }
        for path in &watched {
            watcher
                .watch(path, RecursiveMode::NonRecursive)
                .map_err(|error| {
                    format!(
                        "could not watch settings directory {}: {error}",
                        path.display()
                    )
                })?;
        }

        let (stop_tx, stop_rx) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("settings-watch".into())
            .spawn(move || watch_events(watcher, paths, watched, events_rx, stop_rx, wake))
            .map_err(|error| format!("could not start settings watcher: {error}"))?;
        Ok(Self {
            stop: Some(stop_tx),
            worker: Some(worker),
        })
    }
}

impl Drop for SettingsWatch {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn directories_to_watch(paths: &AppPaths) -> Vec<PathBuf> {
    [paths.config.as_path(), paths.legacy_config.as_path()]
        .into_iter()
        .filter_map(nearest_existing_directory)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}

fn nearest_existing_directory(file: &Path) -> Option<PathBuf> {
    let mut candidate = file.parent()?;
    loop {
        if candidate.is_dir() {
            return Some(candidate.to_path_buf());
        }
        candidate = candidate.parent()?;
    }
}

fn watch_events(
    mut watcher: RecommendedWatcher,
    paths: AppPaths,
    mut watched: Vec<PathBuf>,
    events: Receiver<notify::Result<notify::Event>>,
    stop: Receiver<()>,
    wake: impl Fn() + Send + Sync + 'static,
) {
    loop {
        match events.recv_timeout(Duration::from_millis(50)) {
            Ok(event) => {
                let mut failed = event.is_err();
                while let Ok(next) = events.try_recv() {
                    failed |= next.is_err();
                }
                refresh_watches(&mut watcher, &paths, &mut watched, &mut failed);
                // Debounce until the event stream is quiet; a wake marks the whole burst complete.
                loop {
                    match events.recv_timeout(COALESCE_WINDOW) {
                        Ok(_) => {
                            let mut failed = false;
                            while let Ok(next) = events.try_recv() {
                                failed |= next.is_err();
                            }
                            refresh_watches(&mut watcher, &paths, &mut watched, &mut failed);
                            if failed {
                                eprintln!(
                                    "settings watcher reported a filesystem notification error"
                                );
                            }
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            if failed {
                                eprintln!(
                                    "settings watcher reported a filesystem notification error"
                                );
                            }
                            wake();
                            break;
                        }
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                    if stop.try_recv().is_ok() {
                        return;
                    }
                }
            }
            Err(RecvTimeoutError::Timeout) => {
                if stop.try_recv().is_ok() {
                    return;
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

fn refresh_watches(
    watcher: &mut RecommendedWatcher,
    paths: &AppPaths,
    watched: &mut Vec<PathBuf>,
    failed: &mut bool,
) {
    let desired = directories_to_watch(paths);
    watched.retain(|old| {
        if desired.contains(old) {
            return true;
        }
        match watcher.unwatch(old) {
            Ok(()) => false,
            Err(error) => {
                *failed = true;
                eprintln!(
                    "could not stop watching settings directory {}: {error}",
                    old.display()
                );
                true
            }
        }
    });
    for new in &desired {
        if watched.contains(new) {
            continue;
        }
        match watcher.watch(new, RecursiveMode::NonRecursive) {
            Ok(()) => watched.push(new.clone()),
            Err(error) => {
                *failed = true;
                eprintln!(
                    "could not watch settings directory {}: {error}",
                    new.display()
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{app_paths::AppPaths, settings_watch::SettingsWatch};
    use std::{
        path::PathBuf,
        sync::mpsc::{self, Receiver},
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    struct Temp(PathBuf);

    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "regie-settings-watch-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let path = path.with_extension(format!(
                "{}",
                NEXT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn paths(&self) -> AppPaths {
            AppPaths::new(&self.0)
        }
    }

    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    static NEXT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn watcher(temp: &Temp) -> (SettingsWatch, Receiver<()>) {
        let (send, receive) = mpsc::channel();
        let watch = SettingsWatch::new(temp.paths(), move || {
            let _ = send.send(());
        })
        .unwrap();
        (watch, receive)
    }

    fn wake(receive: &Receiver<()>) {
        receive
            .recv_timeout(Duration::from_secs(3))
            .expect("watcher did not wake before deadline");
    }

    #[test]
    fn wakes_for_write_and_atomic_rename() {
        let temp = Temp::new();
        std::fs::create_dir_all(temp.paths().config.parent().unwrap()).unwrap();
        let (_watch, receive) = watcher(&temp);
        std::fs::write(&temp.paths().config, "height_percent=50").unwrap();
        wake(&receive);
        let replacement = temp.paths().config.with_extension("new");
        std::fs::write(&replacement, "height_percent=60").unwrap();
        std::fs::rename(&replacement, &temp.paths().config).unwrap();
        wake(&receive);
    }

    #[test]
    fn wakes_for_delete_and_recreate() {
        let temp = Temp::new();
        std::fs::create_dir_all(temp.paths().config.parent().unwrap()).unwrap();
        std::fs::write(&temp.paths().config, "height_percent=50").unwrap();
        let (_watch, receive) = watcher(&temp);
        std::fs::remove_file(&temp.paths().config).unwrap();
        wake(&receive);
        std::fs::write(&temp.paths().config, "height_percent=60").unwrap();
        wake(&receive);
    }

    #[test]
    fn wakes_for_canonical_creation_while_legacy_is_present() {
        let temp = Temp::new();
        std::fs::create_dir_all(temp.paths().legacy_config.parent().unwrap()).unwrap();
        std::fs::write(&temp.paths().legacy_config, "height_percent=50").unwrap();
        let (_watch, receive) = watcher(&temp);
        std::fs::create_dir_all(temp.paths().config.parent().unwrap()).unwrap();
        std::fs::write(&temp.paths().config, "height_percent=60").unwrap();
        wake(&receive);
    }

    #[test]
    fn burst_of_changes_is_coalesced() {
        let temp = Temp::new();
        std::fs::create_dir_all(temp.paths().config.parent().unwrap()).unwrap();
        let (_watch, receive) = watcher(&temp);
        for value in 50..60 {
            std::fs::write(&temp.paths().config, format!("height_percent={value}")).unwrap();
        }
        wake(&receive);
        // Drain notification bursts, then ensure no additional wake remains in the bounded window.
        while receive.try_recv().is_ok() {}
        assert!(receive.recv_timeout(Duration::from_millis(250)).is_err());
    }

    #[test]
    fn drop_stops_future_wakes() {
        let temp = Temp::new();
        std::fs::create_dir_all(temp.paths().config.parent().unwrap()).unwrap();
        let (watch, receive) = watcher(&temp);
        drop(watch);
        std::fs::write(&temp.paths().config, "height_percent=50").unwrap();
        assert!(receive.recv_timeout(Duration::from_millis(250)).is_err());
    }
}
