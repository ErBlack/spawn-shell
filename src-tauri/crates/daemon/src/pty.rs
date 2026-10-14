use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
#[serde(from = "RawSize")]
pub struct Size {
    pub rows: u16,
    pub cols: u16,
}

#[derive(Deserialize)]
struct RawSize {
    rows: u16,
    cols: u16,
}

impl From<RawSize> for Size {
    fn from(raw: RawSize) -> Self {
        Size::new(raw.rows, raw.cols)
    }
}

impl Size {
    pub fn new(rows: u16, cols: u16) -> Self {
        Self {
            rows: rows.max(1),
            cols: cols.max(2),
        }
    }

    fn winsize(self) -> libc::winsize {
        libc::winsize {
            ws_row: self.rows,
            ws_col: self.cols,
            ws_xpixel: 0,
            ws_ypixel: 0,
        }
    }
}

pub fn check(result: libc::c_int) -> io::Result<libc::c_int> {
    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(result)
    }
}

pub fn set_cloexec(fd: RawFd, enabled: bool) -> io::Result<()> {
    let flags = check(unsafe { libc::fcntl(fd, libc::F_GETFD) })?;
    let flags = if enabled {
        flags | libc::FD_CLOEXEC
    } else {
        flags & !libc::FD_CLOEXEC
    };
    check(unsafe { libc::fcntl(fd, libc::F_SETFD, flags) }).map(drop)
}

unsafe extern "C" {
    fn ptsname_r(fd: libc::c_int, buffer: *mut libc::c_char, length: libc::size_t) -> libc::c_int;
}

fn open(size: Size) -> io::Result<(OwnedFd, OwnedFd)> {
    let flags = libc::O_RDWR | libc::O_NOCTTY | libc::O_CLOEXEC;
    let master = check(unsafe { libc::posix_openpt(flags) })?;
    let master = unsafe { OwnedFd::from_raw_fd(master) };
    check(unsafe { libc::grantpt(master.as_raw_fd()) })?;
    check(unsafe { libc::unlockpt(master.as_raw_fd()) })?;
    let mut name: [libc::c_char; 128] = [0; 128];
    check(unsafe { ptsname_r(master.as_raw_fd(), name.as_mut_ptr(), name.len()) })?;
    let slave = check(unsafe { libc::open(name.as_ptr(), flags) })?;
    let slave = unsafe { OwnedFd::from_raw_fd(slave) };
    resize(master.as_raw_fd(), size)?;
    Ok((master, slave))
}

pub struct Spawn<'a> {
    pub program: &'a str,
    pub args: &'a [String],
    pub cwd: &'a Path,
    pub env: &'a [(&'a str, &'a str)],
    pub size: Size,
}

pub fn spawn(spec: &Spawn) -> io::Result<(i32, OwnedFd)> {
    let (master, slave) = open(spec.size)?;
    let mut command = Command::new(spec.program);
    command
        .args(spec.args)
        .current_dir(spec.cwd)
        .stdin(Stdio::from(slave.try_clone()?))
        .stdout(Stdio::from(slave.try_clone()?))
        .stderr(Stdio::from(slave))
        .envs(spec.env.iter().copied());
    unsafe {
        command.pre_exec(|| {
            let mut signals: libc::sigset_t = std::mem::zeroed();
            libc::sigemptyset(&raw mut signals);
            let unblocked =
                libc::pthread_sigmask(libc::SIG_SETMASK, &raw const signals, std::ptr::null_mut());
            if unblocked != 0 {
                return Err(io::Error::from_raw_os_error(unblocked));
            }
            check(libc::setsid())?;
            check(libc::ioctl(0, libc::TIOCSCTTY.into(), 0))?;
            Ok(())
        });
    }
    let child = command.spawn()?;
    Ok((child.id().cast_signed(), master))
}

pub fn resize(master: RawFd, size: Size) -> io::Result<()> {
    let winsize = size.winsize();
    check(unsafe { libc::ioctl(master, libc::TIOCSWINSZ, &winsize) }).map(drop)
}

pub fn foreground(master: RawFd) -> Option<i32> {
    let group = unsafe { libc::tcgetpgrp(master) };
    (group > 0).then_some(group)
}

pub fn signal(pid: i32, signal: libc::c_int) {
    unsafe {
        libc::kill(pid, signal);
    }
}

pub fn child_running(pid: i32) -> bool {
    let mut status = 0;
    unsafe { libc::waitpid(pid, &raw mut status, libc::WNOHANG) == 0 }
}

pub fn exists(pid: i32) -> bool {
    unsafe { libc::kill(pid, 0) == 0 }
}

pub fn reap(pid: i32) {
    let mut status = 0;
    unsafe {
        libc::waitpid(pid, &raw mut status, 0);
    }
}

fn argv0(pid: i32) -> Option<String> {
    let mut mib = [libc::CTL_KERN, libc::KERN_PROCARGS2, pid];
    let mut size: libc::size_t = 0;
    let query = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            std::ptr::null_mut(),
            &raw mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if query != 0 || size == 0 {
        return None;
    }
    let mut buffer = vec![0u8; size];
    let read = unsafe {
        libc::sysctl(
            mib.as_mut_ptr(),
            3,
            buffer.as_mut_ptr().cast(),
            &raw mut size,
            std::ptr::null_mut(),
            0,
        )
    };
    if read != 0 {
        return None;
    }
    let (argc, rest) = buffer.get(..size)?.split_first_chunk::<4>()?;
    if i32::from_ne_bytes(*argc) == 0 {
        return None;
    }
    let path_end = rest.iter().position(|byte| *byte == 0)?;
    let rest = &rest[path_end..];
    let start = rest.iter().position(|byte| *byte != 0)?;
    let rest = &rest[start..];
    let end = rest.iter().position(|byte| *byte == 0)?;
    let argv0 = std::str::from_utf8(&rest[..end]).ok()?;
    let name = argv0.rsplit('/').next()?.trim_start_matches('-');
    (!name.is_empty()).then(|| name.to_string())
}

pub fn process_name(pid: i32) -> Option<String> {
    if let Some(name) = argv0(pid) {
        return Some(name);
    }
    let mut path = [0u8; 4096];
    let capacity = u32::try_from(path.len()).ok()?;
    let written = unsafe { libc::proc_pidpath(pid, path.as_mut_ptr().cast(), capacity) };
    let written = usize::try_from(written).ok().filter(|&length| length > 0)?;
    let path = std::str::from_utf8(&path[..written]).ok()?;
    Some(path.rsplit('/').next()?.to_string())
}

pub fn process_cwd(pid: i32) -> Option<PathBuf> {
    let mut info: libc::proc_vnodepathinfo = unsafe { std::mem::zeroed() };
    let size = libc::c_int::try_from(std::mem::size_of::<libc::proc_vnodepathinfo>()).ok()?;
    let written = unsafe {
        libc::proc_pidinfo(
            pid,
            libc::PROC_PIDVNODEPATHINFO,
            0,
            (&raw mut info).cast(),
            size,
        )
    };
    if written < size {
        return None;
    }

    let raw = &info.pvi_cdir.vip_path;
    let bytes = unsafe { std::slice::from_raw_parts(raw.as_ptr().cast::<u8>(), size_of_val(raw)) };
    let end = bytes.iter().position(|byte| *byte == 0)?;
    let path = std::str::from_utf8(&bytes[..end]).ok()?;
    Some(PathBuf::from(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_pty_closes_on_exec_and_has_its_size() {
        let (master, slave) = open(Size::new(30, 100)).unwrap();
        for fd in [master.as_raw_fd(), slave.as_raw_fd()] {
            let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
            assert_ne!(flags & libc::FD_CLOEXEC, 0);
        }
        let mut winsize = Size::new(1, 2).winsize();
        unsafe { libc::ioctl(slave.as_raw_fd(), libc::TIOCGWINSZ, &raw mut winsize) };
        assert_eq!((winsize.ws_row, winsize.ws_col), (30, 100));
    }

    #[test]
    fn a_stored_size_is_never_empty() {
        let size: Size = serde_json::from_str(r#"{"rows":0,"cols":0}"#).unwrap();
        assert_eq!(size, Size::new(1, 2));
    }

    #[test]
    fn process_name_is_the_name_it_was_started_by() {
        let dir = std::env::temp_dir().join(format!("spawnshell-argv-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let link = dir.join("claude");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink("/bin/sleep", &link).unwrap();

        let mut child = Command::new(&link).arg("5").spawn().unwrap();
        let name = process_name(child.id().cast_signed());
        let _ = child.kill();
        let _ = child.wait();
        let _ = std::fs::remove_dir_all(&dir);

        assert_eq!(name.as_deref(), Some("claude"));
    }
}
