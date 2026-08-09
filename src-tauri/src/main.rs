mod config;
mod native;
mod pty;
mod shortcut;
mod window;

use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::ShortcutState;

#[tauri::command]
fn report_fatal(app: AppHandle, message: String) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
    let handle = app.clone();
    app.dialog()
        .message(message)
        .title("Summon failed to start")
        .kind(MessageDialogKind::Error)
        .show(move |_| handle.exit(1));
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            window::show(app);
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        window::toggle(app);
                    }
                })
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(pty::PtyState::default())
        .manage(window::WindowState::default())
        .invoke_handler(tauri::generate_handler![
            pty::pty_spawn,
            pty::pty_write,
            pty::pty_resize,
            window::hide_window,
            window::is_open,
            config::get_config,
            report_fatal,
        ])
        .setup(|app| {
            let config = config::load();
            if shortcut::apply_shortcut(app.handle(), &config.shortcut).is_err() {
                use tauri_plugin_dialog::{DialogExt, MessageDialogKind};
                app.dialog()
                    .message(format!(
                        "Couldn't register the global shortcut \"{}\" — it may be in use by another app.",
                        config.shortcut
                    ))
                    .title("Shortcut unavailable")
                    .kind(MessageDialogKind::Warning)
                    .show(|_| {});
            }
            app.manage(config);

            app.set_activation_policy(tauri::ActivationPolicy::Accessory);
            window::setup_panel(app.handle());
            window::watch_screen_changes(app.handle());

            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::Focused(false) => window::hide(window.app_handle()),
            tauri::WindowEvent::CloseRequested { api, .. } => {
                api.prevent_close();
                window::hide(window.app_handle());
            }
            _ => {}
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
