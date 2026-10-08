//! OS locks coordinate cooperating engines. Publication also rejects non-cooperating writers.
use std::{
    fmt,
    fs::{self, File, OpenOptions, TryLockError},
    io,
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum OwnershipError {
    Busy { path: PathBuf },
    Io { path: PathBuf, source: io::Error },
}

impl fmt::Display for OwnershipError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Busy { path } => write!(
                f,
                "another engine owns {}; close that job or wait, then Resume/Retry",
                path.display()
            ),
            Self::Io { path, source } => {
                write!(f, "cannot acquire ownership of {}: {source}", path.display())
            }
        }
    }
}

impl std::error::Error for OwnershipError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Busy { .. } => None,
            Self::Io { source, .. } => Some(source),
        }
    }
}

/// Keep this handle alive for the entire write operation. Never unlink a lock file:
/// deleting it would let a new inode be locked while the old owner is still active.
#[derive(Debug)]
pub struct ExclusivePathLock {
    _file: File,
}

impl ExclusivePathLock {
    fn acquire(path: &Path) -> Result<Self, OwnershipError> {
        let io_error = |source| OwnershipError::Io {
            path: path.to_path_buf(),
            source,
        };
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.custom_flags(rustix::fs::OFlags::NOFOLLOW.bits() as i32);
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            // Open the reparse point itself, then reject non-regular handles below.
            options.custom_flags(0x0020_0000);
        }
        let file = options.open(path).map_err(io_error)?;
        if !file.metadata().map_err(io_error)?.file_type().is_file() {
            return Err(io_error(io::Error::new(
                io::ErrorKind::InvalidInput,
                "the ownership marker must be a regular file",
            )));
        }
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(TryLockError::WouldBlock) => Err(OwnershipError::Busy {
                path: path.to_path_buf(),
            }),
            Err(TryLockError::Error(source)) => Err(io_error(source)),
        }
    }

    pub(crate) fn database(path: &Path) -> Result<(Self, PathBuf), OwnershipError> {
        let io_error = |source| OwnershipError::Io {
            path: path.to_path_buf(),
            source,
        };
        let canonical = match fs::canonicalize(path) {
            Ok(path) => path,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
                let parent = fs::canonicalize(parent.unwrap_or_else(|| Path::new(".")))
                    .map_err(io_error)?;
                let name = path.file_name().ok_or_else(|| {
                    io_error(io::Error::new(io::ErrorKind::InvalidInput, "invalid database path"))
                })?;
                parent.join(name)
            }
            Err(source) => return Err(io_error(source)),
        };
        let mut marker = canonical.as_os_str().to_os_string();
        marker.push(".pipeline.lock");
        Ok((Self::acquire(Path::new(&marker))?, canonical))
    }
}

/// Acquire canonical, distinct roots without blocking. A failed group releases
/// all handles already acquired and leaves state/attempt counts unchanged.
#[derive(Debug)]
pub struct OutputOwnership {
    _locks: Vec<ExclusivePathLock>,
}

impl OutputOwnership {
    pub fn acquire(roots: &[&Path]) -> Result<Self, OwnershipError> {
        let mut paths = Vec::new();
        for root in roots {
            let io_error = |source| OwnershipError::Io {
                path: root.to_path_buf(),
                source,
            };
            fs::create_dir_all(root).map_err(io_error)?;
            paths.push(fs::canonicalize(root).map_err(io_error)?);
        }
        paths.sort();
        paths.dedup();
        let mut locks = Vec::with_capacity(paths.len());
        for path in paths {
            locks.push(ExclusivePathLock::acquire(&path.join(".pipeline-owner.lock"))?);
        }
        Ok(Self { _locks: locks })
    }
}

/// Publish a whole staged directory or file on the same filesystem, without
/// replacing any existing destination, including empty directories and symlinks.
/// If the OS/filesystem does not support this operation, fail without fallback.
pub fn publish_noreplace(source: &Path, destination: &Path) -> io::Result<()> {
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        use rustix::fs::{renameat_with, RenameFlags, CWD};
        renameat_with(CWD, source, CWD, destination, RenameFlags::NOREPLACE)
            .map_err(io::Error::from)
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        #[link(name = "kernel32")]
        unsafe extern "system" {
            fn MoveFileW(existing: *const u16, new: *const u16) -> i32;
        }
        fn wide(path: &Path) -> io::Result<Vec<u16>> {
            let mut bytes: Vec<u16> = path.as_os_str().encode_wide().collect();
            if bytes.contains(&0) {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, "path contains NUL"));
            }
            bytes.push(0);
            Ok(bytes)
        }
        let source = wide(source)?;
        let destination = wide(destination)?;
        // SAFETY: both owned buffers remain live and are NUL-terminated UTF-16.
        // MoveFileW has no replacement/copy-across-volumes flag.
        if unsafe { MoveFileW(source.as_ptr(), destination.as_ptr()) } == 0 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
    {
        let _ = (source, destination);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "atomic no-replace publication is unsupported on this platform",
        ))
    }
}
