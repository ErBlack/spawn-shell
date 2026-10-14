use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, SystemTime};

use anyhow::{Context, Result};
use config::{Config, Key};
use protocol::Event;

use super::hub::Hub;
use super::report;
use crate::files;

const POLL_INTERVAL: Duration = Duration::from_secs(1);
const SETTLE_DELAY: Duration = Duration::from_millis(200);

type Stamp = Option<(SystemTime, u64)>;

pub(super) struct Settings {
    pub(super) config: Config,
    pub(super) problem: Option<String>,
    loaded: Stamp,
}

fn read(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("can't read {}", path.display())),
    }
}

fn reload(path: &Path) -> Result<Option<Config>> {
    read(path)?
        .map(|text| config::parse(path, &text))
        .transpose()
}

fn load(path: &Path) -> Result<Config> {
    if let Some(config) = reload(path)? {
        return Ok(config);
    }
    let config = Config::default();
    files::write(path, config::render(&config)?.as_bytes())?;
    Ok(config)
}

fn set(path: &Path, key: Key, text: &str) -> Result<Config> {
    let current = read(path)?.unwrap_or_default();
    let (text, config) = config::edit(path, &current, key, text)?;
    files::write(path, text.as_bytes())?;
    Ok(config)
}

impl Settings {
    pub(super) fn load() -> Self {
        let path = config::path();
        let loaded = path.as_deref().ok().and_then(stamp);
        match path.and_then(|path| load(&path)) {
            Ok(config) => Settings {
                config,
                problem: None,
                loaded,
            },
            Err(error) => {
                report(&error);
                Settings {
                    config: Config::default(),
                    problem: Some(format!("{error:#}")),
                    loaded,
                }
            }
        }
    }
}

fn stamp(path: &Path) -> Stamp {
    let metadata = fs::metadata(path).ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

impl Hub {
    pub(super) fn config(&self) -> Config {
        self.settings.lock().config.clone()
    }

    fn apply_config(&self, settings: &mut Settings, config: Config) {
        settings.problem = None;
        if settings.config != config {
            settings.config = config.clone();
            self.notify(Event::Config { config });
        }
    }

    pub(super) fn set_config(&self, key: Key, value: &str) -> Result<()> {
        let mut settings = self.settings.lock();
        let config = set(&config::path()?, key, value)?;
        self.apply_config(&mut settings, config);
        Ok(())
    }

    fn reload_config(&self, path: &Path) {
        let mut settings = self.settings.lock();
        match reload(path) {
            Ok(None) => {}
            Ok(Some(config)) => self.apply_config(&mut settings, config),
            Err(error) => {
                report(&error);
                let message = format!("{error:#}");
                if settings.problem.as_ref() != Some(&message) {
                    settings.problem = Some(message.clone());
                    self.notify(Event::Warning { message });
                }
            }
        }
    }

    pub(super) fn watch_config(self: &Arc<Self>) -> Result<()> {
        let path: PathBuf = config::path()?;
        let mut seen = self.settings.lock().loaded;
        let hub = Arc::clone(self);
        thread::spawn(move || {
            loop {
                thread::sleep(POLL_INTERVAL);
                let now = stamp(&path);
                if now == seen {
                    continue;
                }
                thread::sleep(SETTLE_DELAY);
                if stamp(&path) == now {
                    seen = now;
                    hub.reload_config(&path);
                }
            }
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("spawnshell-config-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir.join("config.toml")
    }

    #[test]
    fn a_missing_file_gets_the_defaults() {
        let path = scratch("missing");
        let config = load(&path).unwrap();
        let written = load(&path).unwrap();
        let _ = fs::remove_dir_all(path.parent().unwrap());

        assert_eq!(config, Config::default());
        assert_eq!(written, Config::default());
    }

    #[test]
    fn invalid_values_change_nothing() {
        let path = scratch("invalid");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, "height = 70\n").unwrap();

        let size = set(&path, Key::FontSize, "100");
        let shortcut = set(&path, Key::Shortcut, "Control+Nothing");
        let text = fs::read_to_string(&path).unwrap();
        let _ = fs::remove_dir_all(path.parent().unwrap());

        assert!(size.is_err());
        assert!(shortcut.is_err());
        assert_eq!(text, "height = 70\n");
    }
}
