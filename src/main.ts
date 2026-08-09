import { Channel, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { openUrl } from "@tauri-apps/plugin-opener";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { Terminal } from "@xterm/xterm";
import { setupWindow } from "./window.ts";
import "./styles.css";

interface Config {
  font_family: string;
  font_size: number;
}

let termRef: Terminal | undefined;

async function main() {
  const config = await invoke<Config>("get_config");

  const terminal = new Terminal({
    fontFamily: `${config.font_family}, monospace`,
    fontSize: Math.min(Math.max(config.font_size, 6), 40),
    cursorBlink: true,
    cursorStyle: "bar",
    drawBoldTextInBrightColors: true,
    allowTransparency: true,
    theme: {
      background: "#00000000",
      foreground: "#e6edf3",
      cursor: "#2f81f7",
      cursorAccent: "#6fc1ff",
      selectionBackground: "#e6edf3",
      selectionForeground: "#0d1117",
      black: "#484f58",
      red: "#ff7b72",
      green: "#3fb950",
      yellow: "#d29922",
      blue: "#58a6ff",
      magenta: "#bc8cff",
      cyan: "#39c5cf",
      white: "#b1bac4",
      brightBlack: "#6e7681",
      brightRed: "#ffa198",
      brightGreen: "#56d364",
      brightYellow: "#e3b341",
      brightBlue: "#79c0ff",
      brightMagenta: "#d2a8ff",
      brightCyan: "#56d4dd",
      brightWhite: "#ffffff",
      scrollbarSliderBackground: "#00000000",
      scrollbarSliderHoverBackground: "#00000000",
      scrollbarSliderActiveBackground: "#00000000",
    },
  });
  termRef = terminal;

  terminal.loadAddon(
    new WebLinksAddon((event, uri) => {
      if (event.metaKey) openUrl(uri);
    }),
  );

  const pane = document.getElementById("terminal");
  if (!pane) throw new Error("#terminal not found");
  terminal.open(pane);

  function fit() {
    if (!pane) return;
    const screen = pane.querySelector<HTMLElement>(".xterm-screen");
    if (!screen || terminal.rows < 1 || terminal.cols < 1) return;
    const cellHeight = screen.clientHeight / terminal.rows;
    const cellWidth = screen.clientWidth / terminal.cols;
    if (cellHeight <= 0 || cellWidth <= 0) return;
    const style = getComputedStyle(pane);
    const availHeight =
      pane.clientHeight -
      parseFloat(style.paddingTop) -
      parseFloat(style.paddingBottom);
    const availWidth =
      pane.clientWidth -
      parseFloat(style.paddingLeft) -
      parseFloat(style.paddingRight);
    const rows = Math.max(1, Math.floor(availHeight / cellHeight));
    const cols = Math.max(2, Math.floor(availWidth / cellWidth));
    if (rows !== terminal.rows || cols !== terminal.cols) {
      terminal.resize(cols, rows);
    }
  }

  function spawnPty() {
    const output = new Channel<ArrayBuffer>();
    output.onmessage = (data) => terminal.write(new Uint8Array(data));
    return invoke("pty_spawn", {
      rows: terminal.rows,
      cols: terminal.cols,
      onOutput: output,
    });
  }

  const reason = (error: unknown) =>
    error instanceof Error ? error.message : String(error);
  const showError = (text: string) =>
    terminal.write(`\r\n\x1b[31m${text}\x1b[0m\r\n`);

  const encoder = new TextEncoder();
  let inputFailed = false;
  const write = (bytes: Uint8Array) =>
    invoke("pty_write", bytes).then(
      () => {
        inputFailed = false;
      },
      (error: unknown) => {
        if (inputFailed) return;
        inputFailed = true;
        showError(`Summon couldn't send input: ${reason(error)}`);
      },
    );
  terminal.onData((data) => write(encoder.encode(data)));
  terminal.onBinary((data) =>
    write(Uint8Array.from(data, (char) => char.charCodeAt(0))),
  );

  let exited = false;
  await listen("pty://exit", () => {
    exited = true;
  });

  async function spawnPtyOrReport() {
    try {
      await spawnPty();
    } catch (error) {
      exited = true;
      showError(`Summon couldn't start the shell: ${reason(error)}`);
    }
  }

  await setupWindow(pane, {
    fitAndResize() {
      fit();
      invoke("pty_resize", { rows: terminal.rows, cols: terminal.cols });
    },
    focus() {
      terminal.focus();
    },
    ensureAlive() {
      if (!exited) return;
      exited = false;
      terminal.reset();
      spawnPtyOrReport();
    },
  });

  fit();
  await spawnPtyOrReport();
}

main().catch((error) => {
  const message = error instanceof Error ? error.message : String(error);
  if (termRef) {
    termRef.write(`\r\n\x1b[31mSummon failed to start: ${message}\x1b[0m\r\n`);
    return;
  }
  invoke("report_fatal", { message });
});
