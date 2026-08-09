use tauri::AppHandle;
use tauri_plugin_global_shortcut::GlobalShortcutExt;

pub fn apply_shortcut(app: &AppHandle, accelerator: &str) -> Result<(), String> {
    let global_shortcut = app.global_shortcut();
    global_shortcut
        .unregister_all()
        .map_err(|error| error.to_string())?;
    global_shortcut
        .register(accelerator)
        .map_err(|error| error.to_string())
}
