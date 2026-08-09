use std::io::{Read, Write};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;

use parking_lot::Mutex;
use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};
use tauri::ipc::{Channel, InvokeBody, InvokeResponseBody, Request};
use tauri::{AppHandle, Emitter, Manager, State};

struct Pty {
    id: u64,
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn Child + Send + Sync>,
    output: Arc<Mutex<Channel<InvokeResponseBody>>>,
    parser: Arc<Mutex<vt100::Parser>>,
}

impl Drop for Pty {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[derive(Default)]
pub struct PtyState {
    pty: Mutex<Option<Pty>>,
    pending: Mutex<Vec<u8>>,
    next_id: AtomicU64,
}

const MAX_PENDING_INPUT: usize = 64 * 1024;

pub fn discard_pending_input(app: &AppHandle) {
    app.state::<PtyState>().pending.lock().clear();
}

fn user_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string())
}

fn pty_size(rows: u16, cols: u16) -> PtySize {
    PtySize {
        rows,
        cols,
        pixel_width: 0,
        pixel_height: 0,
    }
}

#[tauri::command]
pub fn pty_spawn(
    app: AppHandle,
    state: State<'_, PtyState>,
    rows: u16,
    cols: u16,
    on_output: Channel<InvokeResponseBody>,
) -> Result<(), String> {
    let mut guard = state.pty.lock();
    if let Some(pty) = guard.as_mut()
        && matches!(pty.child.try_wait(), Ok(None)) {
            let mut output = pty.output.lock();
            let mut parser = pty.parser.lock();
            parser.screen_mut().set_size(rows, cols);
            let snapshot = parser.screen().state_formatted();
            drop(parser);
            *output = on_output;
            let channel = output.clone();
            drop(output);
            let _ = pty.master.resize(pty_size(rows, cols));
            let _ = channel.send(InvokeResponseBody::Raw(snapshot));
            return Ok(());
        }

    let pair = native_pty_system()
        .openpty(pty_size(rows, cols))
        .map_err(|error| error.to_string())?;

    let config_command = app.state::<crate::config::Config>().command.clone();
    let mut command = CommandBuilder::new(user_shell());
    command.arg("-l");
    if !config_command.trim().is_empty() {
        command.arg("-c");
        command.arg(&config_command);
    }
    command.env("TERM", "xterm-256color");
    command.env("COLORTERM", "truecolor");
    if let Ok(home) = app.path().home_dir() {
        command.cwd(home);
    }

    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|error| error.to_string())?;
    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|error| error.to_string())?;
    let mut writer = pair
        .master
        .take_writer()
        .map_err(|error| error.to_string())?;

    let id = state.next_id.fetch_add(1, Ordering::Relaxed);
    let output = Arc::new(Mutex::new(on_output));
    let output_for_reader = Arc::clone(&output);
    let parser = Arc::new(Mutex::new(vt100::Parser::new(rows, cols, 0)));
    let parser_for_reader = Arc::clone(&parser);
    let app_for_reader = app.clone();
    thread::spawn(move || {
        let mut buffer = [0u8; 8192];
        loop {
            match reader.read(&mut buffer) {
                Ok(0) => break,
                Ok(count) => {
                    let channel = {
                        let guard = output_for_reader.lock();
                        parser_for_reader.lock().process(&buffer[..count]);
                        guard.clone()
                    };
                    let _ = channel.send(InvokeResponseBody::Raw(buffer[..count].to_vec()));
                }
                Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => break,
            }
        }

        let state = app_for_reader.state::<PtyState>();
        let mut slot = state.pty.lock();
        let was_current = slot.as_ref().is_some_and(|pty| pty.id == id);
        if was_current {
            *slot = None;
        }
        drop(slot);

        if was_current {
            let _ = app_for_reader.emit("pty://exit", ());
            crate::window::hide(&app_for_reader);
        }
    });

    let mut pending = state.pending.lock();
    if !pending.is_empty() {
        let _ = writer.write_all(&pending);
        let _ = writer.flush();
        pending.clear();
    }
    drop(pending);

    *guard = Some(Pty {
        id,
        master: pair.master,
        writer,
        child,
        output,
        parser,
    });
    Ok(())
}

#[tauri::command]
pub fn pty_write(state: State<'_, PtyState>, request: Request<'_>) -> Result<(), String> {
    let InvokeBody::Raw(data) = request.body() else {
        return Err("expected raw input bytes".to_string());
    };
    let mut guard = state.pty.lock();
    match guard.as_mut() {
        Some(pty) => {
            pty.writer
                .write_all(data)
                .map_err(|error| error.to_string())?;
            pty.writer.flush().map_err(|error| error.to_string())?;
        }
        None => {
            let mut pending = state.pending.lock();
            if pending.len() + data.len() > MAX_PENDING_INPUT {
                return Err("no terminal is running".to_string());
            }
            pending.extend_from_slice(data);
        }
    }
    Ok(())
}

#[tauri::command]
pub fn pty_resize(state: State<'_, PtyState>, rows: u16, cols: u16) -> Result<(), String> {
    let guard = state.pty.lock();
    if let Some(pty) = guard.as_ref() {
        pty.parser.lock().screen_mut().set_size(rows, cols);
        pty.master
            .resize(pty_size(rows, cols))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
