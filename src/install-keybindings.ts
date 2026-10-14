import type { TabManager } from "./tab-manager.ts";

function combo(event: KeyboardEvent): string {
  const parts: string[] = [];
  if (event.metaKey) parts.push("cmd");
  if (event.ctrlKey) parts.push("ctrl");
  if (event.altKey) parts.push("alt");
  if (event.shiftKey) parts.push("shift");
  parts.push(event.code.toLowerCase());
  return parts.join("+");
}

export function installKeybindings(manager: TabManager): void {
  const bindings: Record<string, () => void> = {
    "cmd+keyt": () => manager.create(),
    "cmd+keyw": () => manager.closeActive(),
    "cmd+shift+bracketleft": () => manager.previous(),
    "cmd+shift+bracketright": () => manager.next(),
    "ctrl+tab": () => manager.next(),
    "ctrl+shift+tab": () => manager.previous(),
  };

  document.addEventListener(
    "keydown",
    (event) => {
      if (event.isComposing) return;
      const key = combo(event);

      if (/^cmd\+digit[1-9]$/.test(key)) {
        event.preventDefault();
        const digit = Number(event.code.slice(-1));
        if (digit === 9) manager.selectLast();
        else manager.selectIndex(digit - 1);
        return;
      }

      const action = bindings[key];
      if (!action) return;
      event.preventDefault();
      action();
    },
    { capture: true },
  );
}
