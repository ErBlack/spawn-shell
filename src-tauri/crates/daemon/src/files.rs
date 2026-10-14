use std::fs::{self, File};
use std::io::{self, ErrorKind, Write};
use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::report;

fn checked(result: libc::c_int) -> io::Result<()> {
    if result == -1 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn retrying(call: impl Fn() -> io::Result<()>) -> io::Result<()> {
    loop {
        match call() {
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            result => return result,
        }
    }
}

fn sync_with(
    full: impl Fn() -> io::Result<()>,
    plain: impl Fn() -> io::Result<()>,
) -> io::Result<()> {
    retrying(full).or_else(|_| retrying(plain))
}

fn sync(file: &File) -> io::Result<()> {
    let fd = file.as_raw_fd();
    sync_with(
        || checked(unsafe { libc::fcntl(fd, libc::F_FULLFSYNC) }),
        || checked(unsafe { libc::fsync(fd) }),
    )
}

fn resolve(path: &Path) -> io::Result<PathBuf> {
    match fs::canonicalize(path) {
        Ok(target) => Ok(target),
        Err(error) if error.kind() == ErrorKind::NotFound && !path.is_symlink() => {
            Ok(path.to_path_buf())
        }
        Err(error) => Err(error),
    }
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let target = resolve(path)?;
    let dir = target.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let mut name = target.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    let staging = target.with_file_name(name);
    let mut file = File::create(&staging)?;
    let staged = (|| {
        if let Ok(metadata) = fs::metadata(&target) {
            file.set_permissions(metadata.permissions())?;
        }
        file.write_all(bytes)?;
        sync(&file)?;
        fs::rename(&staging, &target)
    })();
    if staged.is_err() {
        let _ = fs::remove_file(&staging);
    }
    staged?;
    if let Err(error) = File::open(dir).and_then(|dir| sync(&dir)) {
        report(&anyhow::Error::new(error).context(format!("can't sync {}", dir.display())));
    }
    Ok(())
}

pub fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    write_synced(path, bytes).with_context(|| format!("can't write {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writing_through_a_symlink_keeps_the_link() {
        let dir = std::env::temp_dir().join(format!("spawnshell-files-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let real = dir.join("real.toml");
        let link = dir.join("link.toml");
        fs::write(&real, "old").unwrap();
        let _ = fs::remove_file(&link);
        std::os::unix::fs::symlink(&real, &link).unwrap();

        write(&link, b"new").unwrap();
        let is_link = fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink();
        let content = fs::read_to_string(&real).unwrap();
        let _ = fs::remove_dir_all(&dir);

        assert!(is_link);
        assert_eq!(content, "new");
    }

    #[test]
    fn writing_keeps_permissions_and_dangling_links() {
        use std::os::unix::fs::PermissionsExt;

        let dir =
            std::env::temp_dir().join(format!("spawnshell-files-mode-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("config.toml");
        fs::write(&file, "old").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        let dangling = dir.join("dangling.toml");
        let _ = fs::remove_file(&dangling);
        std::os::unix::fs::symlink(dir.join("missing").join("target.toml"), &dangling).unwrap();

        write(&file, b"new").unwrap();
        let mode = fs::metadata(&file).unwrap().permissions().mode() & 0o777;
        let refused = write(&dangling, b"new").is_err();
        let still_link = dangling.is_symlink();
        let _ = fs::remove_dir_all(&dir);

        assert_eq!(mode, 0o600);
        assert!(refused);
        assert!(still_link);
    }

    #[test]
    fn syncing_falls_back_when_a_full_sync_is_unsupported() {
        use std::cell::Cell;

        let plain = Cell::new(0);
        let fell_back = sync_with(
            || Err(io::Error::from(ErrorKind::Unsupported)),
            || {
                plain.set(plain.get() + 1);
                Ok(())
            },
        );
        assert!(fell_back.is_ok());
        assert_eq!(plain.get(), 1);

        let interrupted = Cell::new(true);
        let full = sync_with(
            || {
                if interrupted.replace(false) {
                    Err(io::Error::from(ErrorKind::Interrupted))
                } else {
                    Ok(())
                }
            },
            || {
                plain.set(plain.get() + 1);
                Ok(())
            },
        );
        assert!(full.is_ok());
        assert_eq!(plain.get(), 1);
    }
}
