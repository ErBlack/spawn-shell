use std::sync::atomic::{AtomicBool, Ordering};

use objc2::MainThreadMarker;
use tauri::{AppHandle, Emitter, Manager, WebviewWindow};

#[derive(Default)]
pub struct WindowState {
    pub open: AtomicBool,
}

fn position_window(app: &AppHandle, window: &WebviewWindow) {
    let percent = app.state::<crate::config::Config>().height.clamp(10, 100);
    if let (Ok(ptr), Some(mtm)) = (window.ns_window(), MainThreadMarker::new()) {
        crate::native::position_at_cursor(mtm, ptr, percent);
    }
}

pub fn show(app: &AppHandle) {
    crate::pty::discard_pending_input(app);
    app.state::<WindowState>()
        .open
        .store(true, Ordering::Relaxed);
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(window) = handle.get_webview_window("main") else {
            return;
        };
        position_window(&handle, &window);
        if let (Ok(ptr), Some(mtm)) = (window.ns_window(), MainThreadMarker::new()) {
            crate::native::show(mtm, ptr);
        }
        let _ = window.emit("summon://show", ());
    });
}

pub fn hide(app: &AppHandle) {
    if !app
        .state::<WindowState>()
        .open
        .swap(false, Ordering::Relaxed)
    {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.emit("summon://hide", ());
    }
}

pub fn toggle(app: &AppHandle) {
    if app.state::<WindowState>().open.load(Ordering::Relaxed) {
        hide(app);
    } else {
        show(app);
    }
}

pub fn reposition_if_open(app: &AppHandle) {
    if !app.state::<WindowState>().open.load(Ordering::Relaxed) {
        return;
    }
    if let Some(window) = app.get_webview_window("main") {
        position_window(app, &window);
    }
}

pub fn setup_panel(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main")
        && let (Ok(ptr), Some(mtm)) = (window.ns_window(), MainThreadMarker::new()) {
            crate::native::make_panel(mtm, ptr);
        }
}

pub fn watch_screen_changes(app: &AppHandle) {
    use block2::RcBlock;
    use objc2_app_kit::NSApplicationDidChangeScreenParametersNotification;
    use objc2_foundation::{NSNotification, NSNotificationCenter};

    let handle = app.clone();
    let block = RcBlock::new(move |_: core::ptr::NonNull<NSNotification>| {
        reposition_if_open(&handle);
    });
    // The observer and block are intentionally leaked: the watcher must live for
    // the whole process lifetime, and there is nothing to unregister it from.
    unsafe {
        let observer = NSNotificationCenter::defaultCenter()
            .addObserverForName_object_queue_usingBlock(
                Some(NSApplicationDidChangeScreenParametersNotification),
                None,
                None,
                &block,
            );
        std::mem::forget(observer);
    }
    std::mem::forget(block);
}

#[tauri::command]
pub fn is_open(app: AppHandle) -> bool {
    app.state::<WindowState>().open.load(Ordering::Relaxed)
}

#[tauri::command]
pub fn hide_window(app: AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if handle.state::<WindowState>().open.load(Ordering::Relaxed) {
            return;
        }
        if let Some(window) = handle.get_webview_window("main")
            && let (Ok(ptr), Some(mtm)) = (window.ns_window(), MainThreadMarker::new()) {
                crate::native::order_out(mtm, ptr);
            }
    });
}
