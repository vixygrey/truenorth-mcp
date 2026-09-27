//! Serialization and repository ownership for runtime mutations.
//!
//! One coordinator belongs to one server context. Its async mutex serializes every
//! read-modify-write transaction in that process. Production coordinators also retain an
//! advisory OS lock for the lifetime of the server, so only one server mutates a worktree.

use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;
use tokio::sync::{Mutex, MutexGuard};

use crate::engine::agent_ws::AGENT_DIR;

/// The worktree-relative writer lease path.
pub const WRITER_LOCK_REL_PATH: &str = ".agent/runtime/writer.lock";

/// A process-local mutation serializer with an optional repository lease.
#[derive(Debug)]
pub struct MutationCoordinator {
    repo_root: PathBuf,
    state: Mutex<LeaseState>,
}

/// The state protected by the process-local mutation gate.
#[derive(Debug)]
enum LeaseState {
    /// Deterministic test contexts serialize locally without touching disk.
    LocalOnly,
    /// This process owns the repository lease.
    Held { _lease: WorktreeLease },
    /// Another process owned the lease at the last acquisition attempt.
    Contended(LeaseConflict),
    /// The lease could not be opened or locked.
    Unavailable(String),
}

/// An open descriptor that owns the advisory worktree writer lease.
#[derive(Debug)]
pub struct WorktreeLease {
    _file: File,
}

/// Diagnostic information from an active lease owner.
#[derive(Debug, Clone)]
struct LeaseConflict {
    holder_pid: Option<u32>,
}

/// Permission to execute one complete mutation transaction.
#[derive(Debug)]
pub struct MutationPermit<'a> {
    _guard: MutexGuard<'a, LeaseState>,
}

/// A repository-lease failure returned before a mutation reads state.
#[derive(Debug, Error)]
pub enum MutationError {
    /// Another process owns the worktree's writer lease.
    #[error(
        "another TrueNorth server owns the writer lease `{lock_path}`{holder}. Use that server or use a separate Git worktree."
    )]
    Contended {
        /// The repository-relative lock path.
        lock_path: &'static str,
        /// The active owner's process id when readable.
        holder_pid: Option<u32>,
        /// A display suffix for the optional holder pid.
        holder: String,
    },
    /// The lease path could not be created, opened, or locked.
    #[error(
        "could not acquire the TrueNorth writer lease `{lock_path}`: {detail}. Check `.agent/` permissions or use a separate Git worktree."
    )]
    Unavailable {
        /// The repository-relative lock path.
        lock_path: &'static str,
        /// The underlying failure.
        detail: String,
    },
}

/// Acquire the same worktree writer lease used by mutating MCP tools.
pub fn acquire_worktree_lease(repo_root: &Path) -> Result<WorktreeLease, MutationError> {
    try_acquire_lease(repo_root).map_err(|error| match error {
        AcquireError::Contended(conflict) => {
            let holder_pid = conflict.holder_pid;
            let holder = holder_pid
                .map(|pid| format!(" (process {pid})"))
                .unwrap_or_default();
            MutationError::Contended {
                lock_path: WRITER_LOCK_REL_PATH,
                holder_pid,
                holder,
            }
        }
        AcquireError::Unavailable(detail) => MutationError::Unavailable {
            lock_path: WRITER_LOCK_REL_PATH,
            detail,
        },
    })
}

impl Default for MutationCoordinator {
    fn default() -> Self {
        Self::local_only(PathBuf::new())
    }
}

impl MutationCoordinator {
    /// Build a production coordinator and attempt to claim the worktree immediately.
    pub fn for_repo(repo_root: PathBuf) -> Self {
        let state = match try_acquire_lease(&repo_root) {
            Ok(lease) => LeaseState::Held { _lease: lease },
            Err(AcquireError::Contended(conflict)) => LeaseState::Contended(conflict),
            Err(AcquireError::Unavailable(error)) => LeaseState::Unavailable(error),
        };
        Self {
            repo_root,
            state: Mutex::new(state),
        }
    }

    /// Build an injected coordinator that performs only in-process serialization.
    pub fn local_only(repo_root: PathBuf) -> Self {
        Self {
            repo_root,
            state: Mutex::new(LeaseState::LocalOnly),
        }
    }

    /// Acquire permission for one complete mutation transaction.
    ///
    /// A contending server retries the OS lease here. This lets an already-running reader
    /// become the writer after the previous owner exits or crashes.
    pub async fn begin(&self) -> Result<MutationPermit<'_>, MutationError> {
        let mut guard = self.state.lock().await;
        if matches!(
            *guard,
            LeaseState::Contended(_) | LeaseState::Unavailable(_)
        ) {
            *guard = match try_acquire_lease(&self.repo_root) {
                Ok(lease) => LeaseState::Held { _lease: lease },
                Err(AcquireError::Contended(conflict)) => LeaseState::Contended(conflict),
                Err(AcquireError::Unavailable(error)) => LeaseState::Unavailable(error),
            };
        }

        match &*guard {
            LeaseState::LocalOnly | LeaseState::Held { .. } => Ok(MutationPermit { _guard: guard }),
            LeaseState::Contended(conflict) => {
                let holder_pid = conflict.holder_pid;
                let holder = holder_pid
                    .map(|pid| format!(" (process {pid})"))
                    .unwrap_or_default();
                Err(MutationError::Contended {
                    lock_path: WRITER_LOCK_REL_PATH,
                    holder_pid,
                    holder,
                })
            }
            LeaseState::Unavailable(detail) => Err(MutationError::Unavailable {
                lock_path: WRITER_LOCK_REL_PATH,
                detail: detail.clone(),
            }),
        }
    }

    /// Describe the initial repository lease state for startup diagnostics.
    pub fn initial_status(&self) -> &'static str {
        match self.state.try_lock() {
            Ok(guard) => match &*guard {
                LeaseState::LocalOnly => "local-only",
                LeaseState::Held { .. } => "held",
                LeaseState::Contended(_) => "contended",
                LeaseState::Unavailable(_) => "unavailable",
            },
            Err(_) => "busy",
        }
    }
}

#[derive(Debug)]
enum AcquireError {
    Contended(LeaseConflict),
    Unavailable(String),
}

fn try_acquire_lease(repo_root: &Path) -> Result<WorktreeLease, AcquireError> {
    #[cfg(not(unix))]
    {
        let _ = repo_root;
        return Err(AcquireError::Unavailable(
            "repository leases require a supported Unix runtime".to_string(),
        ));
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        use std::os::unix::io::AsRawFd;

        let agent_root = repo_root.join(AGENT_DIR);
        let runtime_dir = agent_root.join("runtime");
        std::fs::create_dir_all(&runtime_dir)
            .map_err(|error| AcquireError::Unavailable(error.to_string()))?;

        let canonical_agent = agent_root
            .canonicalize()
            .map_err(|error| AcquireError::Unavailable(error.to_string()))?;
        let canonical_runtime = runtime_dir
            .canonicalize()
            .map_err(|error| AcquireError::Unavailable(error.to_string()))?;
        if !canonical_runtime.starts_with(&canonical_agent) {
            return Err(AcquireError::Unavailable(
                "the runtime lease directory escapes `.agent/` through a symlink".to_string(),
            ));
        }

        let lock_path = runtime_dir.join("writer.lock");
        if std::fs::symlink_metadata(&lock_path).is_ok_and(|metadata| metadata.is_symlink()) {
            return Err(AcquireError::Unavailable(
                "the writer lock path is a symlink".to_string(),
            ));
        }

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&lock_path)
            .map_err(|error| AcquireError::Unavailable(error.to_string()))?;

        // SAFETY: `file` owns a valid open descriptor for the lock file. `flock` does not
        // retain the raw descriptor beyond this call. The open `File` remains alive in
        // `WorktreeLease` for the full lease lifetime.
        let locked = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
        if locked != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::EWOULDBLOCK) {
                return Err(AcquireError::Contended(read_holder(&lock_path)));
            }
            return Err(AcquireError::Unavailable(error.to_string()));
        }

        let acquired_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let metadata = serde_json::json!({
            "pid": std::process::id(),
            "acquired_at_unix": acquired_at,
        })
        .to_string();
        file.set_len(0)
            .and_then(|()| file.seek(SeekFrom::Start(0)).map(|_| ()))
            .and_then(|()| file.write_all(metadata.as_bytes()))
            .and_then(|()| file.sync_data())
            .map_err(|error| AcquireError::Unavailable(error.to_string()))?;

        Ok(WorktreeLease { _file: file })
    }
}

fn read_holder(path: &Path) -> LeaseConflict {
    let mut text = String::new();
    let holder_pid = File::open(path)
        .and_then(|mut file| file.read_to_string(&mut text))
        .ok()
        .and_then(|_| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|value| value.get("pid").and_then(serde_json::Value::as_u64))
        .and_then(|pid| u32::try_from(pid).ok());
    LeaseConflict { holder_pid }
}
