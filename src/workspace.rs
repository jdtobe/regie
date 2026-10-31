//! Application policy. Window/menu/clipboard operations are effects executed on the UI thread.
use crate::app_paths::AppPaths;
use crate::app_ui::AppCommand;
use crate::close_policy::{CloseDecision, CloseIntent, ClosePolicy};
use crate::config::{Config, LastTabAction, NewTabDirectory};
use crate::process::{JobState, SessionObservation};
use crate::pty::{default_login_shell, ShellSpec, TerminalSize};
use crate::session_output::SessionOutput;
use crate::sessions::Sessions;
use crate::tab_layout;
use crate::tabs::{TabId, TabName, Tabs};
use crate::terminal_protocol::{TabSummary, TerminalDelivery, UiDelivery, WorkspaceRequest};
use std::collections::BTreeMap;
use std::path::PathBuf;

pub enum WorkspaceCommand {
    Start,
    Summon,
    Pump,
    Native(AppCommand),
    Ui(WorkspaceRequest),
    Confirm { token: u64, confirmed: bool },
    Paste { id: TabId, text: String },
}
#[derive(Debug)]
pub enum WorkspaceEffect {
    Ui(Vec<UiDelivery>),
    ShowPanel,
    HidePanel,
    Confirm { token: u64, names: Vec<String> },
    Exit,
    Copy(String),
    ReadPaste(TabId),
}
pub struct Workspace {
    config: Config,
    home: PathBuf,
    paths: AppPaths,
    shell: ShellSpec,
    tabs: Tabs,
    sessions: Sessions,
    output: SessionOutput,
    close: ClosePolicy,
    observations: BTreeMap<TabId, SessionObservation>,
    ready: bool,
    started: bool,
    quitting: bool,
    exiting: bool,
    notices: Vec<String>,
    effects: Vec<WorkspaceEffect>,
    output_batch: Vec<crate::session_output::SessionDelivery>,
    preserve_saved: bool,
}

impl Workspace {
    pub fn new(config: Config, home: PathBuf, paths: AppPaths, output: SessionOutput) -> Self {
        Self {
            config,
            home,
            paths,
            shell: default_login_shell(std::env::var_os("SHELL")),
            tabs: Tabs::new(),
            sessions: Sessions::new(output.clone()),
            output,
            close: ClosePolicy::default(),
            observations: BTreeMap::new(),
            ready: false,
            started: false,
            quitting: false,
            exiting: false,
            notices: vec![],
            effects: vec![],
            output_batch: vec![],
            preserve_saved: false,
        }
    }
    pub fn confirmation_pending(&self) -> bool {
        self.close.pending()
    }
    pub fn hide_on_focus_loss(&self) -> bool {
        self.config.hide_on_focus_lost && !self.close.pending() && !self.quitting
    }
    pub fn active(&self) -> Option<TabId> {
        self.tabs.active()
    }
    pub fn finish_delivery(&mut self, success: bool) {
        let batch = std::mem::take(&mut self.output_batch);
        if !success && !batch.is_empty() {
            self.output.retry(batch);
            let output = self.output.clone();
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(100));
                output.wake();
            });
        }
    }
    fn ui(&mut self, deliveries: Vec<UiDelivery>) {
        if self.ready && !deliveries.is_empty() {
            self.effects.push(WorkspaceEffect::Ui(deliveries));
        }
    }
    fn notice(&mut self, text: String) {
        eprintln!("{text}");
        if self.notices.len() == 16 {
            self.notices.remove(0);
        }
        self.notices.push(text.clone());
        self.ui(vec![UiDelivery::Notice { text }]);
    }
    fn metadata(&mut self) {
        let tabs = self
            .tabs
            .ordered()
            .iter()
            .map(|tab| TabSummary {
                id: tab.id,
                title: tab.display_name(),
                manual: tab.name.manual.clone(),
                busy: !tab.ended
                    && matches!(
                        self.observations.get(&tab.id).map(|o| o.jobs),
                        Some(JobState::Busy | JobState::Unknown)
                    ),
            })
            .collect();
        self.ui(vec![UiDelivery::TabMetadata {
            tabs,
            active: self.tabs.active(),
        }]);
    }
    fn save(&mut self, fresh: bool) {
        if self.preserve_saved {
            return;
        }
        let observations = if fresh {
            self.tabs
                .ordered()
                .iter()
                .map(|t| (t.id, self.sessions.observe(t.id)))
                .collect::<Vec<_>>()
        } else {
            vec![]
        };
        let saved = tab_layout::snapshot(&self.tabs, &observations);
        if let Err(error) = tab_layout::save_atomic(&self.paths.layout, &saved) {
            self.notice(error);
        }
    }
    fn spawn_tab(&mut self, cwd: PathBuf, name: TabName) -> Option<TabId> {
        let id = match self.tabs.reserve_id() {
            Ok(id) => id,
            Err(error) => {
                self.notice(error);
                return None;
            }
        };
        let mut shell = self.shell.clone();
        shell.cwd = Some(cwd.clone());
        let size = TerminalSize::new(80, 24).unwrap();
        let cwd = match self.sessions.spawn(id, shell.clone(), size) {
            Ok(()) => cwd,
            Err(error) if cwd != self.home => {
                self.notice(format!("{error}; trying home instead"));
                shell.cwd = Some(self.home.clone());
                if let Err(error) = self.sessions.spawn(id, shell, size) {
                    self.notice(error);
                    return None;
                }
                self.home.clone()
            }
            Err(error) => {
                self.notice(error);
                return None;
            }
        };
        self.tabs.insert(id, cwd, name);
        self.ui(vec![
            UiDelivery::CreateTab { id },
            UiDelivery::SelectTab { id },
        ]);
        self.metadata();
        Some(id)
    }
    fn new_tab(&mut self, cwd_override: Option<PathBuf>) {
        let cwd = if self.config.new_tab_directory == NewTabDirectory::Home {
            self.home.clone()
        } else {
            cwd_override
                .or_else(|| {
                    self.tabs.active().and_then(|id| {
                        let cwd = self
                            .sessions
                            .observe(id)
                            .cwd
                            .or_else(|| self.tabs.get(id).map(|t| t.cwd.clone()));
                        if let Some(cwd) = cwd.as_ref() {
                            self.tabs.set_cwd(id, cwd.clone());
                        }
                        cwd
                    })
                })
                .unwrap_or_else(|| self.home.clone())
        };
        if self.spawn_tab(cwd, Default::default()).is_some() {
            self.preserve_saved = false;
            self.save(false);
        }
    }
    fn jobs(&self) -> Vec<(TabId, JobState)> {
        self.tabs
            .ordered()
            .iter()
            .map(|t| {
                (
                    t.id,
                    if t.ended {
                        JobState::Ended
                    } else {
                        self.sessions.observe(t.id).jobs
                    },
                )
            })
            .collect()
    }
    fn request_close(&mut self, mut intent: CloseIntent) {
        if matches!(intent,CloseIntent::Tab(id) if self.tabs.get(id).is_some())
            && self.tabs.ordered().len() == 1
            && self.config.last_tab_action == LastTabAction::Quit
        {
            intent = CloseIntent::Quit;
        }
        let jobs = self.jobs();
        let decision = self.close.request(intent, &self.tabs, &jobs);
        self.decide(decision);
    }
    fn decide(&mut self, decision: CloseDecision) {
        match decision {
            CloseDecision::Ignore => {}
            CloseDecision::Confirm { token, affected } => {
                let names = affected
                    .iter()
                    .filter_map(|id| self.tabs.get(*id).map(|t| t.display_name()))
                    .collect();
                self.effects.push(WorkspaceEffect::Confirm { token, names });
            }
            CloseDecision::Proceed {
                intent: CloseIntent::Tab(id),
            } => {
                self.preserve_saved = false;
                let cwd = self
                    .sessions
                    .observe(id)
                    .cwd
                    .or_else(|| self.tabs.get(id).map(|t| t.cwd.clone()));
                let _ = self.sessions.remove(id);
                self.tabs.remove(id);
                self.observations.remove(&id);
                self.ui(vec![UiDelivery::RemoveTab { id }]);
                if self.tabs.ordered().is_empty() {
                    match self.config.last_tab_action {
                        LastTabAction::New => self.new_tab(cwd),
                        LastTabAction::Hide => self.effects.push(WorkspaceEffect::HidePanel),
                        LastTabAction::Quit => {
                            unreachable!("last-tab quit is coordinated before removal")
                        }
                    }
                } else if let Some(id) = self.tabs.active() {
                    self.ui(vec![UiDelivery::SelectTab { id }]);
                }
                self.metadata();
                self.save(false);
            }
            CloseDecision::Proceed {
                intent: CloseIntent::Quit,
            } => {
                self.save(true);
                self.quitting = true;
                let ids: Vec<_> = self.tabs.ordered().iter().map(|t| t.id).collect();
                for id in ids {
                    let _ = self.sessions.remove(id);
                }
                self.pump();
            }
        }
    }
    fn pump(&mut self) {
        let updates = self.sessions.take_updates();
        let mut cwd_changed = false;
        let mut meta_changed = false;
        for (id, observation) in updates.observations {
            if self.tabs.get(id).is_none() || self.quitting {
                continue;
            }
            if let Some(cwd) = observation.cwd.as_ref() {
                if self.tabs.get(id).is_some_and(|t| t.cwd != *cwd) {
                    self.tabs.set_cwd(id, cwd.clone());
                    cwd_changed = true;
                }
            }
            self.observations.insert(id, observation);
            meta_changed = true;
        }
        let failed = !updates.errors.is_empty();
        for error in updates.errors {
            self.notice(format!("Could not confirm session cleanup: {error}"));
        }
        if self.ready {
            let batch = self.output.take_batch();
            let mut deliveries = vec![];
            for delivery in &batch {
                if self.tabs.get(delivery.id).is_none() {
                    continue;
                }
                if matches!(delivery.event, TerminalDelivery::Exited { .. }) {
                    self.tabs.mark_ended(delivery.id);
                    meta_changed = true;
                } else if matches!(delivery.event, TerminalDelivery::Failed(_)) {
                    self.tabs.mark_failed(delivery.id);
                    meta_changed = true;
                }
                deliveries.push(UiDelivery::from(delivery));
            }
            self.output_batch = batch;
            self.ui(deliveries);
        }
        if meta_changed {
            self.metadata();
        }
        if cwd_changed && !self.quitting {
            self.save(false);
        }
        if self.quitting && failed {
            self.quitting = false;
            let ids: Vec<_> = self.tabs.ordered().iter().map(|t| t.id).collect();
            for id in ids {
                self.tabs.mark_ended(id);
            }
            self.metadata();
        }
        if self.quitting && self.sessions.pending_cleanup() == 0 && !self.exiting {
            // Acquire the final errors after observing zero: a worker can finish
            // between the first update drain and the cleanup-count check.
            let late = self.sessions.take_updates();
            if late.errors.is_empty() {
                self.exiting = true;
                self.effects.push(WorkspaceEffect::Exit);
            } else {
                self.quitting = false;
                for error in late.errors {
                    self.notice(format!("Could not confirm session cleanup: {error}"));
                }
            }
        }
    }
    pub fn handle(&mut self, command: WorkspaceCommand) -> Vec<WorkspaceEffect> {
        self.effects.clear();
        if self.exiting {
            return vec![];
        }
        match command {
            WorkspaceCommand::Start if !self.started => {
                self.started = true;
                let saved = match tab_layout::load_compatible(&self.paths) {
                    Ok(loaded) => {
                        for notice in loaded.notices {
                            self.notice(notice);
                        }
                        loaded.saved
                    }
                    Err(error) => {
                        self.notice(error);
                        None
                    }
                };
                let layout =
                    tab_layout::resolve_startup(self.config.startup_tabs, saved, &self.home);
                for text in layout.notices {
                    self.notice(text);
                }
                let mut successes = vec![];
                let attempted = layout.tabs.len();
                for (index, tab) in layout.tabs.into_iter().enumerate() {
                    if let Some(id) = self.spawn_tab(tab.cwd, tab.name) {
                        successes.push((index, id));
                    }
                }
                if let Some((_, id)) = successes
                    .iter()
                    .min_by_key(|(index, _)| index.abs_diff(layout.selected))
                {
                    self.tabs.select(*id);
                    self.ui(vec![UiDelivery::SelectTab { id: *id }]);
                }
                self.metadata();
                self.preserve_saved = successes.len() != attempted;
                self.save(false);
            }
            WorkspaceCommand::Ui(WorkspaceRequest::Ready {}) => {
                self.ready = true;
                let ids: Vec<_> = self.tabs.ordered().iter().map(|t| t.id).collect();
                for id in ids {
                    self.ui(vec![UiDelivery::CreateTab { id }]);
                }
                if let Some(id) = self.tabs.active() {
                    self.ui(vec![UiDelivery::SelectTab { id }]);
                }
                self.metadata();
                for text in self.notices.clone() {
                    self.ui(vec![UiDelivery::Notice { text }]);
                }
                self.pump();
            }
            WorkspaceCommand::Pump => self.pump(),
            WorkspaceCommand::Ui(WorkspaceRequest::OutputAck { id, sequence }) => {
                self.output.ack(id, sequence)
            }
            WorkspaceCommand::Confirm { token, confirmed } => {
                let jobs = self.jobs();
                let decision = self.close.resolve(token, confirmed, &jobs, &self.tabs);
                self.decide(decision);
            }
            _ if self.close.pending() || self.quitting => {}
            WorkspaceCommand::Summon => {
                if self.tabs.active().is_none() {
                    self.new_tab(None);
                }
                self.effects.push(WorkspaceEffect::ShowPanel);
            }
            WorkspaceCommand::Native(AppCommand::NewTab)
            | WorkspaceCommand::Ui(WorkspaceRequest::NewTab {}) => self.new_tab(None),
            WorkspaceCommand::Native(AppCommand::Quit) => self.request_close(CloseIntent::Quit),
            WorkspaceCommand::Native(AppCommand::CloseActiveTab) => {
                if let Some(id) = self.tabs.active() {
                    self.request_close(CloseIntent::Tab(id));
                }
            }
            WorkspaceCommand::Paste { id, text } => {
                if self.tabs.get(id).is_some_and(|t| !t.ended && !t.failed) {
                    self.ui(vec![UiDelivery::Paste { id, text }]);
                }
            }
            WorkspaceCommand::Ui(request) => match request {
                WorkspaceRequest::SelectTab { id } => {
                    if self.tabs.select(id) {
                        self.ui(vec![UiDelivery::SelectTab { id }]);
                        self.metadata();
                        self.save(false);
                    }
                }
                WorkspaceRequest::SelectNumber { number } => {
                    self.tabs.select_number(number);
                    if let Some(id) = self.tabs.active() {
                        self.ui(vec![UiDelivery::SelectTab { id }]);
                        self.metadata();
                        self.save(false);
                    }
                }
                WorkspaceRequest::CycleTab { backwards } => {
                    self.tabs.cycle(backwards);
                    if let Some(id) = self.tabs.active() {
                        self.ui(vec![UiDelivery::SelectTab { id }]);
                        self.metadata();
                        self.save(false);
                    }
                }
                WorkspaceRequest::CloseTab { id } => self.request_close(CloseIntent::Tab(id)),
                WorkspaceRequest::RenameTab { id, text } => {
                    if self.tabs.get(id).is_some() {
                        self.preserve_saved = false;
                        self.tabs.rename(id, &text);
                        self.metadata();
                        self.save(false);
                    }
                }
                WorkspaceRequest::Title { id, text } => {
                    if self.tabs.get(id).is_some() {
                        self.tabs.set_auto_title(id, &text);
                        self.metadata();
                        self.save(false);
                    }
                }
                WorkspaceRequest::Input { id, data } => {
                    if self.tabs.get(id).is_some_and(|t| !t.ended && !t.failed) {
                        if let Err(error) = self.sessions.write(id, data.as_bytes()) {
                            self.notice(error);
                        }
                    }
                }
                WorkspaceRequest::Resize { id, cols, rows } => {
                    if let Ok(size) = TerminalSize::new(cols, rows) {
                        if let Err(error) = self.sessions.resize(id, size) {
                            self.notice(error);
                        }
                    }
                }
                WorkspaceRequest::Copy { id, data } => {
                    if self.tabs.get(id).is_some() {
                        self.effects.push(WorkspaceEffect::Copy(data));
                    }
                }
                WorkspaceRequest::Paste { id } => {
                    if self.tabs.get(id).is_some_and(|t| !t.ended && !t.failed) {
                        self.effects.push(WorkspaceEffect::ReadPaste(id));
                    }
                }
                WorkspaceRequest::Ready {}
                | WorkspaceRequest::OutputAck { .. }
                | WorkspaceRequest::NewTab {} => unreachable!(),
            },
            WorkspaceCommand::Start => {}
        }
        std::mem::take(&mut self.effects)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::StartupTabs;
    use crate::tab_layout::{SavedLayout, SavedTab};
    use std::time::{Duration, Instant};
    struct Harness {
        w: Workspace,
        dir: PathBuf,
    }
    impl Harness {
        fn new(config: Config, start: bool) -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let dir = std::env::temp_dir().join(format!(
                "rake-workspace-{}-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&dir).unwrap();
            let mut paths = AppPaths::new(&dir);
            paths.layout = dir.join("tabs.json");
            let mut w = Workspace::new(config, PathBuf::from("/"), paths, SessionOutput::new());
            w.shell = ShellSpec {
                program: "/bin/sh".into(),
                args: vec!["-i".into()],
                cwd: None,
            };
            if start {
                w.handle(WorkspaceCommand::Start);
                w.handle(WorkspaceCommand::Ui(WorkspaceRequest::Ready {}));
            }
            Self { w, dir }
        }
        fn active(&self) -> TabId {
            self.w.tabs.active().unwrap()
        }
        fn command(&mut self, cmd: &str, marker: &str) {
            let id = self.active();
            self.w.sessions.write(id, cmd.as_bytes()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(3);
            let mut text = String::new();
            while Instant::now() < deadline {
                for delivery in self.w.output.take_batch() {
                    if let TerminalDelivery::Output(bytes) = delivery.event {
                        text.push_str(&String::from_utf8_lossy(&bytes));
                    }
                    self.w.output.ack(delivery.id, delivery.sequence);
                }
                if text.contains(&format!("{marker}\r\n")) {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!("missing marker {marker}: {text}");
        }
    }
    impl Drop for Harness {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    #[test]
    fn legacy_restore_saves_canonical_without_mutating_old_layout() {
        let mut h = Harness::new(
            Config {
                startup_tabs: StartupTabs::Restore,
                ..Default::default()
            },
            false,
        );
        let old = SavedLayout {
            version: 1,
            selected: 1,
            tabs: vec![
                SavedTab {
                    cwd: "/".into(),
                    name: TabName {
                        manual: Some("first".into()),
                        automatic: Some("shell".into()),
                    },
                },
                SavedTab {
                    cwd: h.dir.clone(),
                    name: TabName {
                        manual: Some("second".into()),
                        automatic: Some("work".into()),
                    },
                },
            ],
        };
        tab_layout::save_atomic(&h.w.paths.legacy_layout, &old).unwrap();
        let before = std::fs::read(&h.w.paths.legacy_layout).unwrap();
        h.w.handle(WorkspaceCommand::Start);
        assert_eq!(h.w.tabs.ordered().len(), 2);
        assert_eq!(
            h.w.tabs.get(h.active()).unwrap().name.manual.as_deref(),
            Some("second")
        );
        let new = tab_layout::load(&h.w.paths.layout).unwrap().unwrap();
        assert_eq!(new.selected, 1);
        assert_eq!(new.tabs, old.tabs);
        assert_eq!(std::fs::read(&h.w.paths.legacy_layout).unwrap(), before);
    }

    #[test]
    fn corrupt_canonical_layout_does_not_restore_legacy() {
        let mut h = Harness::new(
            Config {
                startup_tabs: StartupTabs::Restore,
                ..Default::default()
            },
            false,
        );
        tab_layout::save_atomic(
            &h.w.paths.legacy_layout,
            &SavedLayout {
                version: 1,
                selected: 0,
                tabs: vec![SavedTab {
                    cwd: "/".into(),
                    name: TabName {
                        manual: Some("do not restore".into()),
                        automatic: None,
                    },
                }],
            },
        )
        .unwrap();
        std::fs::write(&h.w.paths.layout, "broken").unwrap();
        h.w.handle(WorkspaceCommand::Start);
        assert_eq!(h.w.tabs.get(h.active()).unwrap().name.manual, None);
        assert!(h.w.notices.iter().any(|n| n.contains("invalid")));
    }
    #[test]
    fn new_tab_uses_current_local_cwd() {
        let mut h = Harness::new(Config::default(), true);
        h.command("stty -echo; cd /private; printf 'changed\\n'\n", "changed");
        h.w.handle(WorkspaceCommand::Native(AppCommand::NewTab));
        assert_eq!(
            h.w.tabs.get(h.active()).unwrap().cwd,
            PathBuf::from("/private")
        );
    }
    #[test]
    fn home_setting_ignores_current_cwd() {
        let mut h = Harness::new(
            Config {
                new_tab_directory: NewTabDirectory::Home,
                ..Default::default()
            },
            true,
        );
        h.command("stty -echo; cd /private; printf 'changed\\n'\n", "changed");
        h.w.handle(WorkspaceCommand::Native(AppCommand::NewTab));
        assert_eq!(h.w.tabs.get(h.active()).unwrap().cwd, PathBuf::from("/"));
    }
    #[test]
    fn failed_spawn_preserves_active_tab() {
        let mut h = Harness::new(Default::default(), true);
        let old = h.active();
        h.w.shell.program = "/missing/shell".into();
        h.w.handle(WorkspaceCommand::Native(AppCommand::NewTab));
        assert_eq!(h.active(), old);
        assert_eq!(h.w.tabs.ordered().len(), 1);
    }
    #[test]
    fn closing_last_new_creates_fresh_shell() {
        let mut h = Harness::new(Default::default(), true);
        let old = h.active();
        h.w.tabs.mark_ended(old);
        h.w.handle(WorkspaceCommand::Ui(WorkspaceRequest::CloseTab { id: old }));
        assert_ne!(h.active(), old);
        assert_eq!(h.w.tabs.ordered().len(), 1);
    }
    #[test]
    fn closing_last_hide_then_summon_creates_one_tab() {
        let mut h = Harness::new(
            Config {
                last_tab_action: LastTabAction::Hide,
                ..Default::default()
            },
            true,
        );
        let id = h.active();
        h.w.tabs.mark_ended(id);
        let effects =
            h.w.handle(WorkspaceCommand::Ui(WorkspaceRequest::CloseTab { id }));
        assert!(effects
            .iter()
            .any(|e| matches!(e, WorkspaceEffect::HidePanel)));
        assert!(h.w.tabs.active().is_none());
        h.w.handle(WorkspaceCommand::Summon);
        assert_eq!(h.w.tabs.ordered().len(), 1);
        h.w.handle(WorkspaceCommand::Summon);
        assert_eq!(h.w.tabs.ordered().len(), 1);
    }
    #[test]
    fn last_tab_quit_uses_quit_coordinator() {
        let mut h = Harness::new(
            Config {
                last_tab_action: LastTabAction::Quit,
                ..Default::default()
            },
            true,
        );
        h.command("stty -echo; sleep 30 & printf 'running\\n'\n", "running");
        let id = h.active();
        let effects =
            h.w.handle(WorkspaceCommand::Ui(WorkspaceRequest::CloseTab { id }));
        assert!(effects
            .iter()
            .any(|e| matches!(e, WorkspaceEffect::Confirm { .. })));
        assert!(!h.w.quitting);
    }
    #[test]
    fn replacement_spawn_failure_leaves_working_new_action() {
        let mut h = Harness::new(Default::default(), true);
        let old = h.active();
        h.w.tabs.mark_ended(old);
        h.w.shell.program = "/missing/shell".into();
        h.w.handle(WorkspaceCommand::Ui(WorkspaceRequest::CloseTab { id: old }));
        assert!(h.w.tabs.active().is_none());
        h.w.shell.program = "/bin/sh".into();
        h.w.handle(WorkspaceCommand::Native(AppCommand::NewTab));
        assert!(h.w.tabs.active().is_some());
    }
    #[test]
    fn cancel_quit_keeps_shells_layout_and_selection() {
        let mut h = Harness::new(Default::default(), true);
        let old = h.active();
        h.command("stty -echo; sleep 30 & printf 'running\\n'\n", "running");
        let before = std::fs::read(h.dir.join("tabs.json")).unwrap();
        let effects = h.w.handle(WorkspaceCommand::Native(AppCommand::Quit));
        let token = effects
            .iter()
            .find_map(|e| {
                if let WorkspaceEffect::Confirm { token, .. } = e {
                    Some(*token)
                } else {
                    None
                }
            })
            .unwrap();
        h.w.handle(WorkspaceCommand::Confirm {
            token,
            confirmed: false,
        });
        assert_eq!(h.active(), old);
        assert!(h.w.sessions.contains(old));
        assert_eq!(std::fs::read(h.dir.join("tabs.json")).unwrap(), before);
    }
    #[test]
    fn paste_and_title_after_switch_keep_original_id() {
        let mut h = Harness::new(Default::default(), true);
        let old = h.active();
        h.w.handle(WorkspaceCommand::Native(AppCommand::NewTab));
        let effects = h.w.handle(WorkspaceCommand::Paste {
            id: old,
            text: "hi".into(),
        });
        assert!(effects.iter().any(|e|matches!(e,WorkspaceEffect::Ui(deliveries) if deliveries.iter().any(|d|matches!(d,UiDelivery::Paste{id,..} if *id==old)))));
        h.w.handle(WorkspaceCommand::Ui(WorkspaceRequest::Title {
            id: old,
            text: "server".into(),
        }));
        assert_eq!(h.w.tabs.get(old).unwrap().display_name(), "server");
        assert_ne!(h.w.tabs.get(h.active()).unwrap().display_name(), "server");
    }
    #[test]
    fn unknown_id_is_ignored() {
        let mut h = Harness::new(Default::default(), true);
        let missing = TabId::new(999).unwrap();
        assert!(h
            .w
            .handle(WorkspaceCommand::Ui(WorkspaceRequest::Paste {
                id: missing
            }))
            .is_empty());
        assert!(h
            .w
            .handle(WorkspaceCommand::Paste {
                id: missing,
                text: "oops".into()
            })
            .is_empty());
    }
    #[test]
    fn partial_restore_keeps_other_tabs_when_one_spawn_fails() {
        let mut h = Harness::new(
            Config {
                startup_tabs: StartupTabs::Restore,
                ..Default::default()
            },
            false,
        );
        let missing = h.dir.join("missing");
        let saved = SavedLayout {
            version: 1,
            selected: 1,
            tabs: vec![
                SavedTab {
                    cwd: missing,
                    name: TabName {
                        manual: Some("first".into()),
                        automatic: None,
                    },
                },
                SavedTab {
                    cwd: "/private".into(),
                    name: Default::default(),
                },
            ],
        };
        tab_layout::save_atomic(&h.dir.join("tabs.json"), &saved).unwrap();
        h.w.handle(WorkspaceCommand::Start);
        assert_eq!(h.w.tabs.ordered().len(), 2);
        assert_eq!(h.w.tabs.ordered()[0].cwd, PathBuf::from("/"));
        assert_eq!(
            h.w.tabs.get(h.active()).unwrap().cwd,
            PathBuf::from("/private")
        );
    }
    #[test]
    fn invalid_cwd_spawn_retries_home_once_with_notice() {
        let mut h = Harness::new(Default::default(), false);
        h.w.spawn_tab(h.dir.join("vanished"), Default::default());
        assert_eq!(h.w.tabs.ordered()[0].cwd, PathBuf::from("/"));
        assert!(!h.w.notices.is_empty());
    }

    #[test]
    fn focus_loss_during_dialog_does_not_hide() {
        let mut h = Harness::new(
            Config {
                hide_on_focus_lost: true,
                ..Default::default()
            },
            true,
        );
        h.command("stty -echo; sleep 30 & printf 'running\\n'\n", "running");
        h.w.handle(WorkspaceCommand::Native(AppCommand::Quit));
        assert!(h.w.confirmation_pending());
        assert!(!h.w.hide_on_focus_loss());
    }

    #[test]
    fn confirmed_quit_saves_layout_and_waits_for_cleanup() {
        let mut h = Harness::new(Default::default(), true);
        h.command("stty -echo; sleep 30 & printf 'running\\n'\n", "running");
        let effects = h.w.handle(WorkspaceCommand::Native(AppCommand::Quit));
        let token = effects
            .iter()
            .find_map(|e| {
                if let WorkspaceEffect::Confirm { token, .. } = e {
                    Some(*token)
                } else {
                    None
                }
            })
            .unwrap();
        let mut effects = h.w.handle(WorkspaceCommand::Confirm {
            token,
            confirmed: true,
        });
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if effects.iter().any(|e| matches!(e, WorkspaceEffect::Exit)) {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "quit cleanup never completed: {:?}",
                h.w.notices
            );
            std::thread::sleep(Duration::from_millis(10));
            effects = h.w.handle(WorkspaceCommand::Pump);
        }
        assert_eq!(h.w.sessions.pending_cleanup(), 0);
        assert_eq!(
            tab_layout::load(&h.dir.join("tabs.json"))
                .unwrap()
                .unwrap()
                .tabs
                .len(),
            1,
            "quit saves before removal"
        );
    }

    #[test]
    fn noisy_tab_keeps_input_and_menu_responsive() {
        let mut h = Harness::new(Default::default(), true);
        h.command("stty -echo; yes noisy & printf 'running\\n'\n", "running");
        let start = Instant::now();
        h.w.handle(WorkspaceCommand::Native(AppCommand::NewTab));
        assert!(start.elapsed() < Duration::from_secs(2));
        let active = h.active();
        h.w.handle(WorkspaceCommand::Ui(WorkspaceRequest::Input {
            id: active,
            data: "printf 'alive\\n'\n".into(),
        }));
        let deadline = Instant::now() + Duration::from_secs(3);
        let mut found = false;
        while Instant::now() < deadline && !found {
            for delivery in h.w.output.take_batch() {
                if let TerminalDelivery::Output(bytes) = delivery.event {
                    if delivery.id == active && String::from_utf8_lossy(&bytes).contains("alive") {
                        found = true;
                    }
                }
                h.w.output.ack(delivery.id, delivery.sequence);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(found, "background output starved active input");
    }

    #[test]
    fn failed_terminal_delivery_does_not_prove_jobs_ended() {
        let mut h = Harness::new(Default::default(), true);
        h.command("stty -echo; sleep 30 & printf 'running\\n'\n", "running");
        let id = h.active();
        h.w.output
            .send(
                id,
                crate::terminal_protocol::TerminalEvent::Failed("bridge failed".into()),
            )
            .unwrap();
        h.w.handle(WorkspaceCommand::Pump);
        let effects = h.w.handle(WorkspaceCommand::Native(AppCommand::Quit));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, WorkspaceEffect::Confirm { .. })),
            "failed output is not proof the app died"
        );
    }

    #[test]
    fn failed_startup_does_not_overwrite_recoverable_layout() {
        let mut h = Harness::new(
            Config {
                startup_tabs: StartupTabs::Restore,
                ..Default::default()
            },
            false,
        );
        let saved = SavedLayout {
            version: 1,
            selected: 0,
            tabs: vec![SavedTab {
                cwd: "/private".into(),
                name: TabName {
                    manual: Some("keep me".into()),
                    automatic: None,
                },
            }],
        };
        let path = h.dir.join("tabs.json");
        tab_layout::save_atomic(&path, &saved).unwrap();
        let before = std::fs::read(&path).unwrap();
        h.w.shell.program = "/missing/shell".into();
        h.w.handle(WorkspaceCommand::Start);
        assert!(h.w.tabs.ordered().is_empty());
        assert_eq!(std::fs::read(&path).unwrap(), before);
        h.w.handle(WorkspaceCommand::Native(AppCommand::Quit));
        assert_eq!(
            std::fs::read(&path).unwrap(),
            before,
            "quitting a failed startup must preserve layout too"
        );
    }
}
