//! One running copy per user, with a way for a second launch to ask it to show.
//!
//! The kernel holds the lock, not the file: a crash, a force quit or a reboot releases it,
//! so a leftover lock file never keeps Rake from starting. The socket lives beside the lock
//! and only the lock holder binds it, which gives the second launch a way to wake the panel.

use std::fs::{File, TryLockError};
use std::io::{self, Read, Write};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

pub const LOCK_FILE: &str = "instance.lock";
pub const SOCKET_FILE: &str = "instance.sock";

/// How long a second launch waits for the first to bind its socket.
const CONNECT_ATTEMPTS: u32 = 20;
const CONNECT_PAUSE: Duration = Duration::from_millis(25);

/// The running copy's claim on the app. Dropping it releases the lock and removes the socket.
pub struct Instance {
    _lock: File,
    socket: PathBuf,
}

/// Try to become the one running copy. `Ok(None)` means another copy already holds the lock.
pub fn claim(dir: &Path) -> io::Result<Option<Instance>> {
    std::fs::create_dir_all(dir)?;
    let lock = File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join(LOCK_FILE))?;
    match lock.try_lock() {
        Ok(()) => Ok(Some(Instance {
            _lock: lock,
            socket: dir.join(SOCKET_FILE),
        })),
        Err(TryLockError::WouldBlock) => Ok(None),
        Err(TryLockError::Error(error)) => Err(error),
    }
}

impl Instance {
    /// Answer show requests on the socket, calling `on_show` for each one. The listener
    /// thread runs for the life of the process.
    pub fn listen<F>(&self, on_show: F) -> io::Result<()>
    where
        F: Fn() + Send + 'static,
    {
        // A previous copy may have died without cleaning up its socket.
        let _ = std::fs::remove_file(&self.socket);
        let listener = UnixListener::bind(&self.socket)?;
        thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                // Consume the existing four-byte request before closing the peer.
                if stream.set_nonblocking(true).is_err() {
                    continue;
                }
                let mut request = [0; 4];
                let mut used = 0;
                for _ in 0..CONNECT_ATTEMPTS {
                    match stream.read(&mut request[used..]) {
                        Ok(0) => break,
                        Ok(count) => {
                            used += count;
                            if used == request.len() {
                                break;
                            }
                        }
                        Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                            thread::sleep(CONNECT_PAUSE)
                        }
                        Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                        Err(_) => break,
                    }
                }
                if used == request.len() && &request == b"show" {
                    on_show();
                }
            }
        });
        Ok(())
    }
}

/// Ask the running copy to show its panel. Retries briefly in case it has not bound yet.
pub fn request_show(dir: &Path) -> io::Result<()> {
    let socket = dir.join(SOCKET_FILE);
    let mut last_error = None;
    for _ in 0..CONNECT_ATTEMPTS {
        match UnixStream::connect(&socket) {
            Ok(mut stream) => return stream.write_all(b"show"),
            Err(error) => {
                last_error = Some(error);
                thread::sleep(CONNECT_PAUSE);
            }
        }
    }
    Err(last_error.unwrap_or_else(|| io::Error::other("no socket")))
}

impl Drop for Instance {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn old_and_new_clients_exclude_each_other_in_both_orders() {
        for old_first in [true, false] {
            // The full macOS TMPDIR plus Application Support exceeds sockaddr_un.
            // A short, uniquely owned test HOME keeps the real compatibility suffix.
            let home = PathBuf::from("/tmp").join(format!(
                "rg-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            std::fs::create_dir(&home).unwrap();
            let old = home.join("Library/Application Support/Rake");
            let new = crate::app_paths::AppPaths::new(&home).runtime;
            let (first, second) = if old_first {
                (&old, &new)
            } else {
                (&new, &old)
            };
            let owner = claim(first).unwrap().unwrap();
            let (tx, rx) = mpsc::channel();
            owner
                .listen(move || {
                    let _ = tx.send(());
                })
                .unwrap();
            assert!(claim(second).unwrap().is_none());
            // Saving new-brand data must never replace the already owned runtime socket.
            let paths = crate::app_paths::AppPaths::new(&home);
            crate::tab_layout::save_atomic(
                &paths.layout,
                &crate::tab_layout::SavedLayout {
                    version: 1,
                    selected: 0,
                    tabs: vec![crate::tab_layout::SavedTab {
                        cwd: home.clone(),
                        name: Default::default(),
                    }],
                },
            )
            .unwrap();
            request_show(second).unwrap();
            rx.recv_timeout(Duration::from_secs(2)).unwrap();
            drop(owner);
            assert!(claim(second).unwrap().is_some());
            std::fs::remove_dir_all(home).unwrap();
        }
    }

    fn temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the clock is past the epoch")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("rake-{name}-{}-{nanos}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        dir
    }

    #[test]
    fn only_one_claim_wins_and_dropping_it_releases_the_lock() {
        let dir = temp_dir("claim");

        let first = claim(&dir).expect("claim").expect("first claim wins");
        assert!(
            claim(&dir).expect("claim").is_none(),
            "a second copy is refused"
        );

        drop(first);
        assert!(
            claim(&dir).expect("claim").is_some(),
            "the lock is released with the claim"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn a_second_launch_wakes_the_running_copy() {
        let dir = temp_dir("listen");
        let held = claim(&dir).expect("claim").expect("first claim wins");
        let (sender, receiver) = mpsc::channel();
        held.listen(move || {
            let _ = sender.send(());
        })
        .expect("listen");

        request_show(&dir).expect("request");
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("the show request arrives");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn listener_keeps_connection_open_until_show_is_written() {
        let dir = temp_dir("write");
        let held = claim(&dir).unwrap().unwrap();
        let (tx, rx) = mpsc::channel();
        held.listen(move || {
            let _ = tx.send(());
        })
        .unwrap();
        let mut client = UnixStream::connect(dir.join(SOCKET_FILE)).unwrap();
        assert!(
            rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "listener must consume the request before closing the peer"
        );
        client.write_all(b"show").unwrap();
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(held);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn listening_replaces_a_stale_socket() {
        let dir = temp_dir("stale");
        let held = claim(&dir).expect("claim").expect("first claim wins");
        std::fs::write(dir.join(SOCKET_FILE), b"stale").expect("stale socket file");

        let (sender, receiver) = mpsc::channel();
        held.listen(move || {
            let _ = sender.send(());
        })
        .expect("listen over the stale file");

        request_show(&dir).expect("request");
        receiver
            .recv_timeout(Duration::from_secs(2))
            .expect("the show request arrives");

        std::fs::remove_dir_all(&dir).ok();
    }
}
