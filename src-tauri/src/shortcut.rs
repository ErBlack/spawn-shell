use tauri::AppHandle;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

pub enum Outcome {
    Registered,
    KeptPrevious(anyhow::Error),
    NoneActive(anyhow::Error),
}

pub fn apply(app: &AppHandle, accelerator: &str, previous: Option<&str>) -> Outcome {
    let shortcuts = app.global_shortcut();
    let attempt = shortcuts
        .unregister_all()
        .and_then(|()| shortcuts.register(accelerator));
    let Err(error) = attempt else {
        return Outcome::Registered;
    };
    match previous.map(|previous| (previous, shortcuts.register(previous))) {
        Some((_, Ok(()))) => Outcome::KeptPrevious(error.into()),
        Some((previous, Err(restore))) => {
            log::warn!("can't restore the shortcut {previous}: {restore}");
            Outcome::NoneActive(error.into())
        }
        None => Outcome::NoneActive(error.into()),
    }
}
