import type { Terminal } from "@xterm/xterm";

export class CommandTitle {
  text = "";
  private readonly onChange: () => void;
  private owned = false;
  private running = false;

  constructor(terminal: Terminal, onChange: () => void) {
    this.onChange = onChange;
    terminal.onTitleChange((title) => {
      this.text = title;
      this.owned = this.running;
      onChange();
    });
    terminal.parser.registerOscHandler(133, (data) => this.mark(data));
  }

  private mark(data: string): boolean {
    if (data.startsWith("C")) this.running = true;
    if (!data.startsWith("D")) return false;
    this.running = false;
    if (this.owned) {
      this.owned = false;
      this.text = "";
      this.onChange();
    }
    return false;
  }
}
