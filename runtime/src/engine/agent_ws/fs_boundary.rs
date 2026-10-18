//! Descriptor-relative access to governed files without following repository symlinks.

use std::ffi::{CStr, CString, OsStr, OsString};
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path};
use std::sync::atomic::{AtomicU64, Ordering};

/// An open repository anchor; descendants are opened relative to directory descriptors.
pub(crate) struct RepositoryFs {
    root: File,
}

impl RepositoryFs {
    pub(crate) fn open(root: &Path) -> io::Result<Self> {
        let root = root.canonicalize()?;
        let root = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(root)?;
        Ok(Self { root })
    }

    fn validate_location(path: &Path) -> io::Result<()> {
        if path.as_os_str().is_empty()
            || path.as_os_str().as_bytes().contains(&b'\\')
            || path.as_os_str().as_bytes().contains(&0)
            || path
                .as_os_str()
                .as_bytes()
                .split(|byte| *byte == b'/')
                .any(|part| part.is_empty() || part == b"." || part == b"..")
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(invalid(
                path,
                "expected a nonempty repository-relative path without traversal",
            ));
        }
        Ok(())
    }

    pub(crate) fn validate_path(path: &Path) -> io::Result<()> {
        Self::validate_location(path)?;
        let mut prefix = std::path::PathBuf::new();
        for part in path.components() {
            prefix.push(part);
            if crate::config::is_secret_path(&prefix) || super::is_excluded_read(&prefix) {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    format!(
                        "refused `{}`: private paths and telemetry are excluded; select a governed file",
                        path.display()
                    ),
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn open_read(&self, path: &Path) -> io::Result<File> {
        Self::validate_path(path)?;
        let (parent, name) = self.parent(path)?;
        let file = open_at(&parent, &name, libc::O_RDONLY | libc::O_NONBLOCK, 0)?;
        if !file.metadata()?.is_file() {
            return Err(invalid(path, "expected a regular file"));
        }
        Ok(file)
    }

    pub(crate) fn read(&self, path: &Path) -> io::Result<Vec<u8>> {
        let mut bytes = Vec::new();
        self.open_read(path)?.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    pub(crate) fn validate_directory(&self, path: &Path) -> io::Result<()> {
        Self::validate_path(path)?;
        self.directory(path, false).map(|_| ())
    }

    pub(crate) fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        Self::validate_path(path)?;
        if !path.starts_with(super::AGENT_DIR) {
            return Err(invalid(path, "directory creation requires .agent/"));
        }
        self.directory(path, true).map(|_| ())
    }

    pub(crate) fn read_dir_names(&self, path: &Path) -> io::Result<Vec<OsString>> {
        Self::validate_path(path)?;
        let directory = self.directory(path, false)?;
        let fd = directory.into_raw_fd();
        // fdopendir takes descriptor ownership only on success.
        let stream = unsafe { libc::fdopendir(fd) };
        if stream.is_null() {
            let error = io::Error::last_os_error();
            drop(unsafe { File::from_raw_fd(fd) });
            return Err(error);
        }
        let stream = DirectoryStream(stream);
        let mut names = Vec::new();
        loop {
            // Clearing errno distinguishes end-of-directory from a failed readdir.
            unsafe { *errno_pointer() = 0 };
            let entry = unsafe { libc::readdir(stream.0) };
            if entry.is_null() {
                let error = io::Error::last_os_error();
                return if error.raw_os_error() == Some(0) {
                    Ok(names)
                } else {
                    Err(error)
                };
            }
            // readdir's name is NUL-terminated and valid until the next call.
            let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
            if name.to_bytes() != b"." && name.to_bytes() != b".." {
                names.push(OsStr::from_bytes(name.to_bytes()).to_owned());
            }
        }
    }

    pub(crate) fn open_lock(&self, path: &Path) -> io::Result<File> {
        require_agent(path)?;
        let (parent, name) = self.parent(path)?;
        regular_target(&parent, &name, true)?;
        let file = open_at(
            &parent,
            &name,
            libc::O_RDWR | libc::O_CREAT | libc::O_NONBLOCK,
            0o600,
        )?;
        if !file.metadata()?.is_file() {
            return Err(invalid(path, "expected a regular writer lock"));
        }
        Ok(file)
    }

    pub(crate) fn write_new(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.write(path, bytes, false)
    }

    pub(crate) fn write_atomic(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.write(path, bytes, true)
    }

    fn write(&self, path: &Path, bytes: &[u8], replace: bool) -> io::Result<()> {
        require_agent(path)?;
        let (parent, name) = self.parent(path)?;
        regular_target(&parent, &name, true)?;
        let (mut temporary, temporary_name) = temporary_file(&parent)?;
        let result = (|| {
            temporary.write_all(bytes)?;
            temporary.sync_all()?;
            regular_target(&parent, &name, true)?;
            // Both names resolve relative to the same pinned parent, never a pathname lookup.
            let result = unsafe {
                if replace {
                    libc::renameat(
                        parent.as_raw_fd(),
                        temporary_name.as_ptr(),
                        parent.as_raw_fd(),
                        name.as_ptr(),
                    )
                } else {
                    libc::linkat(
                        parent.as_raw_fd(),
                        temporary_name.as_ptr(),
                        parent.as_raw_fd(),
                        name.as_ptr(),
                        0,
                    )
                }
            };
            if result != 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        })();
        if result.is_err() || !replace {
            // The temporary name was created exclusively and belongs to this operation.
            unsafe { libc::unlinkat(parent.as_raw_fd(), temporary_name.as_ptr(), 0) };
        }
        result
    }

    pub(crate) fn remove_file(&self, path: &Path) -> io::Result<()> {
        require_agent(path)?;
        let (parent, name) = self.parent(path)?;
        regular_target(&parent, &name, false)?;
        // unlinkat removes the directory entry itself and cannot follow a replacement symlink.
        if unsafe { libc::unlinkat(parent.as_raw_fd(), name.as_ptr(), 0) } != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn parent(&self, path: &Path) -> io::Result<(File, CString)> {
        Self::validate_location(path)?;
        let name = path
            .file_name()
            .ok_or_else(|| invalid(path, "expected a file name"))?;
        let parent = match path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            Some(parent) => self.directory(parent, false)?,
            None => self.root.try_clone()?,
        };
        Ok((parent, c_string(name)?))
    }

    fn directory(&self, path: &Path, create: bool) -> io::Result<File> {
        Self::validate_location(path)?;
        let mut directory = self.root.try_clone()?;
        for part in path.components() {
            let name = c_string(part.as_os_str())?;
            directory = match open_at(&directory, &name, libc::O_RDONLY | libc::O_DIRECTORY, 0) {
                Ok(child) => child,
                Err(error) if create && error.kind() == io::ErrorKind::NotFound => {
                    // mkdirat cannot traverse a substituted parent because its descriptor is pinned.
                    let result =
                        unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o755) };
                    if result != 0 {
                        let error = io::Error::last_os_error();
                        if error.kind() != io::ErrorKind::AlreadyExists {
                            return Err(error);
                        }
                    }
                    open_at(&directory, &name, libc::O_RDONLY | libc::O_DIRECTORY, 0)?
                }
                Err(error) => return Err(error),
            };
        }
        Ok(directory)
    }
}

struct DirectoryStream(*mut libc::DIR);

impl Drop for DirectoryStream {
    fn drop(&mut self) {
        // This stream uniquely owns the directory descriptor transferred to fdopendir.
        unsafe { libc::closedir(self.0) };
    }
}

#[cfg(target_os = "macos")]
unsafe fn errno_pointer() -> *mut libc::c_int {
    unsafe { libc::__error() }
}

#[cfg(not(target_os = "macos"))]
unsafe fn errno_pointer() -> *mut libc::c_int {
    unsafe { libc::__errno_location() }
}

fn require_agent(path: &Path) -> io::Result<()> {
    RepositoryFs::validate_path(path)?;
    if !path.starts_with(super::AGENT_DIR) || path == Path::new(super::AGENT_DIR) {
        return Err(invalid(path, "writes require a target under .agent/"));
    }
    Ok(())
}

fn invalid(path: &Path, reason: &str) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!(
            "refused `{}`: {reason}; use a regular governed file without symlinks",
            path.display()
        ),
    )
}

fn c_string(name: &OsStr) -> io::Result<CString> {
    CString::new(name.as_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains a NUL byte; supply a valid file path",
        )
    })
}

fn open_at(
    parent: &File,
    name: &CString,
    flags: libc::c_int,
    mode: libc::c_uint,
) -> io::Result<File> {
    // name has no separators; parent owns its descriptor for the duration of this call.
    let fd = unsafe {
        libc::openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            flags | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            mode,
        )
    };
    if fd < 0 {
        let error = io::Error::last_os_error();
        return Err(match error.raw_os_error() {
            Some(libc::ELOOP | libc::ENOTDIR) => io::Error::new(
                io::ErrorKind::InvalidInput,
                "refused a symlink or non-directory path component; use regular repository paths",
            ),
            _ => error,
        });
    }
    // A successful openat returns a new owned descriptor.
    Ok(unsafe { File::from_raw_fd(fd) })
}

fn regular_target(parent: &File, name: &CString, allow_absent: bool) -> io::Result<()> {
    let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
    // fstatat initializes metadata on success and does not follow the final component.
    if unsafe {
        libc::fstatat(
            parent.as_raw_fd(),
            name.as_ptr(),
            metadata.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } != 0
    {
        let error = io::Error::last_os_error();
        return if allow_absent && error.kind() == io::ErrorKind::NotFound {
            Ok(())
        } else {
            Err(error)
        };
    }
    // The successful call initialized every field used here.
    let metadata = unsafe { metadata.assume_init() };
    if metadata.st_mode & libc::S_IFMT != libc::S_IFREG {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "refused a symlink or nonregular target; use a regular file under .agent/",
        ));
    }
    Ok(())
}

fn temporary_file(parent: &File) -> io::Result<(File, CString)> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    loop {
        let sequence = NEXT.fetch_add(1, Ordering::Relaxed);
        let name = c_string(OsStr::new(&format!(
            ".truenorth-{}-{sequence}.tmp",
            std::process::id()
        )))?;
        match open_at(
            parent,
            &name,
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL,
            0o600,
        ) {
            Ok(file) => return Ok((file, name)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
#[path = "fs_boundary_tests.rs"]
mod tests;
