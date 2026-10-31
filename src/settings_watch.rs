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

/// A completed configuration-change burst, or a watcher failure that needs user attention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsWatchEvent {
    Changed,
    Failed(String),
}

trait WatchAdapter: Send + 'static {
    fn watch(&mut self, path: &Path) -> Result<(), String>;
    fn unwatch(&mut self, path: &Path) -> Result<(), String>;
}

struct NotifyAdapter(RecommendedWatcher);

impl WatchAdapter for NotifyAdapter {
    fn watch(&mut self, path: &Path) -> Result<(), String> {
        self.0
            .watch(path, RecursiveMode::NonRecursive)
            .map_err(|error| error.to_string())
    }

    fn unwatch(&mut self, path: &Path) -> Result<(), String> {
        self.0.unwatch(path).map_err(|error| error.to_string())
    }
}

impl SettingsWatch {
    pub fn new(
        paths: AppPaths,
        wake: impl Fn(SettingsWatchEvent) + Send + Sync + 'static,
    ) -> Result<Self, String> {
        let (events_tx, events_rx) = mpsc::channel();
        let mut watcher =
            notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                let event = event.map(|_| ()).map_err(|error| error.to_string());
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

        spawn_worker(paths, NotifyAdapter(watcher), watched, events_rx, wake)
    }
}

fn spawn_worker(
    paths: AppPaths,
    backend: impl WatchAdapter,
    watched: Vec<PathBuf>,
    events_rx: Receiver<Result<(), String>>,
    wake: impl Fn(SettingsWatchEvent) + Send + Sync + 'static,
) -> Result<SettingsWatch, String> {
    let (stop_tx, stop_rx) = mpsc::channel();
    let worker = thread::Builder::new()
        .name("settings-watch".into())
        .spawn(move || watch_events(backend, paths, watched, events_rx, stop_rx, wake))
        .map_err(|error| format!("could not start settings watcher: {error}"))?;
    Ok(SettingsWatch {
        stop: Some(stop_tx),
        worker: Some(worker),
    })
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
    mut backend: impl WatchAdapter,
    paths: AppPaths,
    mut watched: Vec<PathBuf>,
    events: Receiver<Result<(), String>>,
    stop: Receiver<()>,
    wake: impl Fn(SettingsWatchEvent) + Send + Sync + 'static,
) {
    loop {
        match events.recv_timeout(Duration::from_millis(50)) {
            Ok(event) => {
                let mut failure = event.err();
                while let Ok(next) = events.try_recv() {
                    collect_failure(&mut failure, next.err());
                }
                collect_failure(
                    &mut failure,
                    refresh_watches(&mut backend, &paths, &mut watched).err(),
                );
                // Debounce until the event stream is quiet; a wake marks the whole burst complete.
                loop {
                    match events.recv_timeout(COALESCE_WINDOW) {
                        Ok(_) => {
                            let mut next_failure = None;
                            while let Ok(next) = events.try_recv() {
                                collect_failure(&mut next_failure, next.err());
                            }
                            collect_failure(
                                &mut next_failure,
                                refresh_watches(&mut backend, &paths, &mut watched).err(),
                            );
                            collect_failure(&mut failure, next_failure);
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            wake(match failure {
                                Some(error) => SettingsWatchEvent::Failed(error),
                                None => SettingsWatchEvent::Changed,
                            });
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
    backend: &mut impl WatchAdapter,
    paths: &AppPaths,
    watched: &mut Vec<PathBuf>,
) -> Result<(), String> {
    let desired = directories_to_watch(paths);
    let mut errors = Vec::new();
    watched.retain(|old| {
        if desired.contains(old) {
            return true;
        }
        match backend.unwatch(old) {
            Ok(()) => false,
            Err(error) => {
                errors.push(format!(
                    "could not stop watching {}: {error}",
                    old.display()
                ));
                true
            }
        }
    });
    for new in &desired {
        if watched.contains(new) {
            continue;
        }
        match backend.watch(new) {
            Ok(()) => watched.push(new.clone()),
            Err(error) => {
                errors.push(format!("could not watch {}: {error}", new.display()));
            }
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}

fn collect_failure(target: &mut Option<String>, next: Option<String>) {
    if let Some(next) = next {
        match target {
            Some(previous) => {
                previous.push_str("; ");
                previous.push_str(&next);
            }
            None => *target = Some(next),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::COALESCE_WINDOW;
    use crate::{
        app_paths::AppPaths,
        settings_watch::{
            directories_to_watch, spawn_worker, SettingsWatch, SettingsWatchEvent, WatchAdapter,
        },
    };
    use std::{
        path::{Path, PathBuf},
        sync::mpsc::{self, Receiver, Sender},
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

    struct TestBackend {
        updates: Sender<PathBuf>,
        fail_watch: Option<PathBuf>,
    }

    impl WatchAdapter for TestBackend {
        fn watch(&mut self, path: &Path) -> Result<(), String> {
            if self.fail_watch.as_deref() == Some(path) {
                return Err("injected watch registration failure".into());
            }
            let _ = self.updates.send(path.to_path_buf());
            Ok(())
        }

        fn unwatch(&mut self, _path: &Path) -> Result<(), String> {
            Ok(())
        }
    }

    fn start_test_watch(
        paths: AppPaths,
        mut backend: TestBackend,
        events_rx: Receiver<Result<(), String>>,
    ) -> (SettingsWatch, Receiver<SettingsWatchEvent>) {
        let watched = directories_to_watch(&paths);
        for path in &watched {
            backend.watch(path).unwrap();
        }
        let (wake_tx, wake_rx) = mpsc::channel();
        let watch = spawn_worker(paths, backend, watched, events_rx, move |event| {
            let _ = wake_tx.send(event);
        })
        .unwrap();
        (watch, wake_rx)
    }

    fn watcher(temp: &Temp) -> (SettingsWatch, Receiver<SettingsWatchEvent>) {
        let (send, receive) = mpsc::channel();
        let watch = SettingsWatch::new(temp.paths(), move |event| {
            let _ = send.send(event);
        })
        .unwrap();
        (watch, receive)
    }

    fn wake(receive: &Receiver<SettingsWatchEvent>) {
        assert!(matches!(
            receive
                .recv_timeout(Duration::from_secs(3))
                .expect("watcher did not wake before deadline"),
            SettingsWatchEvent::Changed
        ));
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
    fn re_registers_new_config_parent_before_later_file_change() {
        let temp = Temp::new();
        let (events_tx, events_rx) = mpsc::channel();
        let (registered_tx, registered_rx) = mpsc::channel();
        let backend = TestBackend {
            updates: registered_tx,
            fail_watch: None,
        };
        let (watch, receive) = start_test_watch(temp.paths(), backend, events_rx);
        std::fs::create_dir_all(temp.paths().config.parent().unwrap()).unwrap();
        events_tx.send(Ok(())).unwrap();
        let deadline = std::time::Instant::now() + Duration::from_secs(3);
        let mut registered_config_parent = false;
        while std::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if registered_rx.recv_timeout(remaining).ok().as_deref() == temp.paths().config.parent()
            {
                registered_config_parent = true;
                break;
            }
        }
        assert!(
            registered_config_parent,
            "new config parent was never registered"
        );
        std::fs::write(&temp.paths().config, "height_percent=50").unwrap();
        events_tx.send(Ok(())).unwrap();
        wake(&receive);
        drop(watch);
    }

    #[test]
    fn async_source_failure_is_reported_without_a_healthy_change_event() {
        let temp = Temp::new();
        let (events_tx, events_rx) = mpsc::channel();
        let backend = TestBackend {
            updates: mpsc::channel().0,
            fail_watch: None,
        };
        let (_watch, receive) = start_test_watch(temp.paths(), backend, events_rx);
        events_tx
            .send(Err("injected notification failure".into()))
            .unwrap();
        assert!(matches!(
            receive.recv_timeout(Duration::from_secs(3)).unwrap(),
            SettingsWatchEvent::Failed(message) if message.contains("injected notification failure")
        ));
        assert!(receive.recv_timeout(COALESCE_WINDOW * 2).is_err());
    }

    #[test]
    fn async_registration_failure_is_reported_not_as_a_healthy_change() {
        let temp = Temp::new();
        let config_parent = temp.paths().config.parent().unwrap().to_path_buf();
        let (events_tx, events_rx) = mpsc::channel();
        let backend = TestBackend {
            updates: mpsc::channel().0,
            fail_watch: Some(config_parent.clone()),
        };
        let (_watch, receive) = start_test_watch(temp.paths(), backend, events_rx);
        std::fs::create_dir_all(&config_parent).unwrap();
        events_tx.send(Ok(())).unwrap();
        assert!(matches!(
            receive.recv_timeout(Duration::from_secs(3)).unwrap(),
            SettingsWatchEvent::Failed(message)
                if message.contains("injected watch registration failure")
        ));
        assert!(receive.recv_timeout(COALESCE_WINDOW * 2).is_err());
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
