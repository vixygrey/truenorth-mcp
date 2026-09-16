//! File watcher that debounces cockpit edits and reports the affected resource URIs.
//!
//! A change to a backing file on disk maps to a resource URI. The watcher coalesces
//! edits within a 200 ms window into a single report, so a burst of writes to one file
//! yields one notification (Requirement 5.6). The consumer emits the MCP
//! `notifications/resources/updated` for each reported URI within 1 second of the change
//! (Requirement 5.3); the resources layer wires that emission in a later task.
//!
//! The debounce logic is a pure [`Debouncer`], so it is unit-testable without touching
//! the filesystem. `spawn_watcher` wires the notify backend to it on a background thread.
//!
//! Requirements: 5.3, 5.6. Design: Part II §6 (resource notification).

// The watcher is consumed by the server wiring and resources layer (tasks 14, 15). It is
// unused until those tasks land, so the module-scoped allow prevents a premature
// dead-code error under `clippy -D warnings`. Remove this allow once task 15 wires it.
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use notify::{Event, RecursiveMode, Watcher};

/// The debounce window. Edits within this window coalesce into one report
/// (Requirement 5.6).
pub const DEBOUNCE_WINDOW: Duration = Duration::from_millis(200);

/// A cockpit resource URI backed by a file on disk (design §5 resources).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResourceUri {
    /// `truenorth://state`, backed by `.agent/tasks/state.yml`.
    State,
    /// `truenorth://cockpit`, backed by the release plan and product boundary.
    Cockpit,
    /// `truenorth://ontology`, backed by `.agent/ontology.yml`.
    Ontology,
    /// `truenorth://conventions`, backed by the coding standards.
    Conventions,
    /// `truenorth://adr`, backed by the `specs/adr/` directory (Requirement 9.3).
    Adr,
}

impl ResourceUri {
    /// The URI string.
    pub fn as_str(self) -> &'static str {
        match self {
            ResourceUri::State => "truenorth://state",
            ResourceUri::Cockpit => "truenorth://cockpit",
            ResourceUri::Ontology => "truenorth://ontology",
            ResourceUri::Conventions => "truenorth://conventions",
            ResourceUri::Adr => "truenorth://adr",
        }
    }
}

/// Map a changed path to the resource URI it backs, when any.
///
/// A skill file or an unrelated path maps to `None`, since no cockpit resource is backed
/// by it. The match is on the trailing path components, so an absolute or repo-relative
/// path both resolve.
pub fn map_path_to_uri(path: &Path) -> Option<ResourceUri> {
    let text = path.to_string_lossy().replace('\\', "/");

    if text.ends_with(".agent/tasks/state.yml") {
        return Some(ResourceUri::State);
    }
    if text.ends_with(".agent/ontology.yml") {
        return Some(ResourceUri::Ontology);
    }
    // The release plan or the relocated product path (Requirements 2.6, 2.12). The
    // watcher no longer watches `specs/product/`; the product path is `.agent/product/`.
    if text.ends_with(".agent/tasks/release-plan.yml") || text.contains(".agent/product/") {
        return Some(ResourceUri::Cockpit);
    }
    if text.ends_with("CONVENTIONS.md") || text.ends_with("conventions.md") {
        return Some(ResourceUri::Conventions);
    }
    // The ADR resource is backed by the specs/adr/ directory (Requirement 9.3).
    if text.contains("specs/adr/") {
        return Some(ResourceUri::Adr);
    }
    None
}

/// The pure debounce core.
///
/// It accumulates the resource URIs affected since the window opened and reports them
/// once the window elapses. It holds no timer and does no I/O, so a test drives it with
/// explicit instants.
#[derive(Debug, Default)]
pub struct Debouncer {
    /// The URIs affected in the current window. A set, so repeated edits to one file
    /// coalesce.
    pending: BTreeSet<ResourceUri>,
    /// The instant the current window opened, or `None` when idle.
    window_start: Option<Instant>,
}

impl Debouncer {
    /// A fresh debouncer with no pending changes.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a changed path at `now`. A path that backs no resource is ignored.
    ///
    /// The first change in an idle debouncer opens the window at `now`.
    pub fn record(&mut self, path: &Path, now: Instant) {
        let Some(uri) = map_path_to_uri(path) else {
            return;
        };
        if self.window_start.is_none() {
            self.window_start = Some(now);
        }
        self.pending.insert(uri);
    }

    /// Report the coalesced URIs when the window has elapsed at `now`, else `None`.
    ///
    /// A report drains the pending set and closes the window, so the next change opens a
    /// new window. The report is empty-safe: it returns `None` when nothing is pending.
    pub fn poll(&mut self, now: Instant) -> Option<Vec<ResourceUri>> {
        let start = self.window_start?;
        if now.duration_since(start) < DEBOUNCE_WINDOW {
            return None;
        }
        self.window_start = None;
        let drained: Vec<ResourceUri> = std::mem::take(&mut self.pending).into_iter().collect();
        if drained.is_empty() {
            None
        } else {
            Some(drained)
        }
    }

    /// Whether any change is pending.
    pub fn has_pending(&self) -> bool {
        !self.pending.is_empty()
    }
}

/// A running watcher. Dropping it stops the watch and joins the worker thread.
pub struct WatchHandle {
    /// The notify backend. Kept alive so the watch continues.
    _watcher: notify::RecommendedWatcher,
    /// A signal that stops the worker thread.
    stop: mpsc::Sender<()>,
    /// The worker thread handle.
    worker: Option<std::thread::JoinHandle<()>>,
}

impl Drop for WatchHandle {
    fn drop(&mut self) {
        // Signal the worker to stop, then join it. A send error means the worker already
        // ended, which is fine.
        let _ = self.stop.send(());
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

/// Watch `repo_root` and invoke `on_change` with the coalesced resource URIs after each
/// debounce window (Requirements 5.3, 5.6).
///
/// The watch is recursive under the repo root. A change that backs no resource is
/// ignored. The callback runs on the worker thread.
///
/// # Errors
///
/// Returns a notify error when the watch cannot start.
pub fn spawn_watcher<F>(repo_root: &Path, on_change: F) -> notify::Result<WatchHandle>
where
    F: Fn(Vec<ResourceUri>) + Send + 'static,
{
    let (event_tx, event_rx) = mpsc::channel::<notify::Result<Event>>();
    let (stop_tx, stop_rx) = mpsc::channel::<()>();

    let mut watcher = notify::recommended_watcher(move |result| {
        // A send error means the receiver was dropped, so the watch is ending.
        let _ = event_tx.send(result);
    })?;
    watcher.watch(repo_root, RecursiveMode::Recursive)?;

    let worker = std::thread::spawn(move || {
        run_debounce_loop(&event_rx, &stop_rx, &on_change);
    });

    Ok(WatchHandle {
        _watcher: watcher,
        stop: stop_tx,
        worker: Some(worker),
    })
}

/// The worker loop: feed notify events into the debouncer and flush after the window.
///
/// It polls the event channel with the debounce window as the timeout, so a settled burst
/// flushes promptly, within 1 second of the change (Requirement 5.3).
fn run_debounce_loop<F>(
    event_rx: &mpsc::Receiver<notify::Result<Event>>,
    stop_rx: &mpsc::Receiver<()>,
    on_change: &F,
) where
    F: Fn(Vec<ResourceUri>),
{
    let mut debouncer = Debouncer::new();

    loop {
        if stop_rx.try_recv().is_ok() {
            return;
        }

        match event_rx.recv_timeout(DEBOUNCE_WINDOW) {
            Ok(Ok(event)) => {
                let now = Instant::now();
                for path in event.paths {
                    debouncer.record(&path, now);
                }
            }
            Ok(Err(_)) => {
                // A watch error is non-fatal. Keep serving and try the next event.
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }

        // Flush whatever settled. A short sleep past the window makes the poll fire.
        if debouncer.has_pending() {
            std::thread::sleep(Duration::from_millis(1));
            if let Some(uris) = debouncer.poll(Instant::now()) {
                on_change(uris);
            }
        }
    }
}

// Tests live in a sibling file to hold this module under the size guidance. The
// `#[path]` include keeps them a child module of `watcher`.
#[cfg(test)]
#[path = "watcher_tests.rs"]
mod tests;
