import { openUrl } from "@tauri-apps/plugin-opener";
import { WebLinksAddon } from "@xterm/addon-web-links";
import { type ITerminalOptions, Terminal } from "@xterm/xterm";
import { commands, type Font } from "./bindings.ts";
import { reportFailure } from "./report-failure.ts";

interface Size {
  rows: number;
  cols: number;
}

const THEME = {
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
};

for (const [name, color] of Object.entries(THEME)) {
  document.documentElement.style.setProperty(`--terminal-${name}`, color);
}

function openLink(event: MouseEvent, uri: string) {
  if (event.metaKey) openUrl(uri);
}

function measure(terminal: Terminal): Size | undefined {
  const pane = terminal.element?.parentElement;
  const screen = terminal.element?.querySelector<HTMLElement>(".xterm-screen");
  const { rows, cols } = terminal;
  if (!pane || !screen || rows < 1 || cols < 1) return undefined;
  const cellHeight = screen.clientHeight / rows;
  const cellWidth = screen.clientWidth / cols;
  if (cellHeight <= 0 || cellWidth <= 0) return undefined;
  const style = getComputedStyle(pane);
  const height =
    pane.clientHeight -
    parseFloat(style.paddingTop) -
    parseFloat(style.paddingBottom);
  const width =
    pane.clientWidth -
    parseFloat(style.paddingLeft) -
    parseFloat(style.paddingRight);
  return {
    rows: Math.max(1, Math.floor(height / cellHeight)),
    cols: Math.max(2, Math.floor(width / cellWidth)),
  };
}

export class Terminals {
  private readonly scrollback: number;
  private readonly open = new Set<Terminal>();
  private font: ITerminalOptions = {};
  private size: Size = { rows: 24, cols: 80 };
  private resized: Promise<void> = Promise.resolve();

  constructor(scrollback: number) {
    this.scrollback = scrollback;
  }

  create(container: HTMLElement): Terminal {
    const terminal = new Terminal({
      scrollback: this.scrollback,
      linkHandler: { activate: openLink },
      ...this.font,
      cursorBlink: true,
      cursorStyle: "bar",
      cursorInactiveStyle: "bar",
      drawBoldTextInBrightColors: true,
      allowTransparency: true,
      theme: THEME,
    });
    terminal.loadAddon(new WebLinksAddon(openLink));
    terminal.open(container);
    terminal.resize(this.size.cols, this.size.rows);
    this.open.add(terminal);
    this.fit();
    return terminal;
  }

  dispose(terminal: Terminal) {
    this.open.delete(terminal);
    terminal.dispose();
  }

  settled(): Promise<void> {
    return this.resized;
  }

  setFont(font: Font) {
    this.font = {
      fontFamily: `${font.family}, monospace`,
      fontSize: font.size,
    };
    for (const terminal of this.open)
      Object.assign(terminal.options, this.font);
    requestAnimationFrame(() => this.fit());
  }

  fit() {
    for (const terminal of this.open) {
      const measured = measure(terminal);
      if (measured) return this.apply(measured);
    }
  }

  private apply(measured: Size) {
    if (measured.rows === this.size.rows && measured.cols === this.size.cols)
      return;
    this.size = measured;
    for (const terminal of this.open)
      terminal.resize(measured.cols, measured.rows);
    this.resized = reportFailure(
      commands.terminalResize(measured.rows, measured.cols),
    );
  }
}
