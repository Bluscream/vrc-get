//! OS-specific functionality.

use std::ffi::OsStr;
use std::fs::OpenOptions;
use std::io;
use std::os::unix::prelude::*;
use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

use nix::libc::{F_UNLCK, F_WRLCK, c_short, flock};

pub(crate) use os_more::start_command;

async fn start_command_posix(_: &OsStr, path: &OsStr, args: &[&OsStr]) -> std::io::Result<()> {
    let mut command = Command::new(path);
    command.args(args);
    os_more::fix_env_variables(&mut command);
    command.process_group(0);
    let mut process = command.spawn()?;
    std::thread::spawn(move || process.wait());
    Ok(())
}

pub(crate) fn is_locked(path: &Path) -> io::Result<bool> {
    let mut lock = flock {
        l_start: 0,
        l_len: 0,
        l_pid: 0,
        // Query for a write lock: it reports both read and write locks held by
        // others. macOS denies l_type: 0, and Linux denies F_UNLCK, with EINVAL.
        l_type: F_WRLCK as c_short,
        l_whence: 0,
    };
    let file = OpenOptions::new().read(true).open(path)?;

    nix::fcntl::fcntl(&file, nix::fcntl::F_GETLK(&mut lock))?;

    if lock.l_type != F_UNLCK as c_short {
        return Ok(true);
    }

    // Unity on Linux takes a BSD lock (flock) instead of a POSIX record lock,
    // and F_GETLK cannot see those, so test for it by trying to take it ourselves.
    match nix::fcntl::Flock::lock(file, nix::fcntl::FlockArg::LockExclusiveNonblock) {
        // We got the lock, so nobody else holds it. Dropping it unlocks again.
        Ok(_lock) => Ok(false),
        Err((_, nix::errno::Errno::EWOULDBLOCK)) => Ok(true),
        Err((_, e)) => Err(e.into()),
    }
}

#[cfg(target_os = "macos")]
#[path = "os_macos.rs"]
mod os_more;

#[cfg(target_os = "linux")]
#[path = "os_linux.rs"]
mod os_more;

pub fn os_info() -> &'static str {
    static OS_INFO: OnceLock<String> = OnceLock::new();
    OS_INFO.get_or_init(os_more::compute_os_info)
}

pub use os_more::initialize;
pub use os_more::is_noexec;
pub use os_more::open_that;

/// Asks the process to quit gracefully, letting it run its own shutdown logic.
pub(crate) fn request_process_quit(process: &sysinfo::Process) -> bool {
    process.kill_with(sysinfo::Signal::Term).unwrap_or(false)
}
