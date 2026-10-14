import type { TabEntry } from "./tab-manager.ts";

interface Button {
  root: HTMLButtonElement;
  caption: HTMLSpanElement;
  dot: HTMLSpanElement;
}

export interface TabsActions {
  select(id: string): void;
  menu(id: string | undefined): void;
  place(id: string, pinned: boolean, position: number): void;
}

function fill(group: HTMLElement, buttons: HTMLElement[]) {
  const same =
    group.children.length === buttons.length &&
    buttons.every((button, index) => group.children[index] === button);
  if (!same) group.replaceChildren(...buttons);
}

export class Tabs {
  private readonly bar: HTMLElement;
  private readonly pinnedGroup: HTMLElement;
  private readonly group: HTMLElement;
  private readonly actions: TabsActions;
  private readonly buttons = new Map<string, Button>();
  private readonly owners = new WeakMap<Element, string>();
  private entries: readonly TabEntry[] = [];
  private active: string | undefined;
  private dragged: string | undefined;

  constructor(
    bar: HTMLElement,
    pinnedGroup: HTMLElement,
    group: HTMLElement,
    actions: TabsActions,
  ) {
    this.bar = bar;
    this.pinnedGroup = pinnedGroup;
    this.group = group;
    this.actions = actions;
    bar.addEventListener("contextmenu", (event) => {
      event.preventDefault();
      const button =
        event.target instanceof Element ? event.target.closest(".tab") : null;
      actions.menu(button ? this.owners.get(button) : undefined);
    });
  }

  render(entries: readonly TabEntry[], active: string | undefined) {
    const switched = active !== this.active;
    this.entries = entries;
    this.active = active;
    for (const id of this.buttons.keys())
      if (!entries.some((entry) => entry.info.id === id))
        this.buttons.delete(id);
    this.draw();
    if (switched && active)
      this.buttons.get(active)?.root.scrollIntoView({
        inline: "nearest",
        block: "nearest",
      });
  }

  private draw() {
    for (const { info, tab } of this.entries) {
      const { root, caption, dot } = this.button(info.id);
      const label = tab.title.text || info.process || "";
      if (info.id === this.active) root.setAttribute("aria-current", "true");
      else root.removeAttribute("aria-current");
      root.toggleAttribute("data-stopped", !info.alive);
      root.title = info.cwd ? `${info.process ?? ""} — ${info.cwd}` : "";
      if (caption.textContent !== label) caption.textContent = label;
      dot.hidden = !tab.dirty || info.id === this.active;
    }

    const roots = this.entries.map(({ info }) => this.button(info.id).root);
    const pinned = this.entries.filter(({ info }) => info.pinned).length;
    fill(this.pinnedGroup, roots.slice(0, pinned));
    fill(this.group, roots.slice(pinned));
    this.bar.toggleAttribute("data-dragging", this.dragged !== undefined);
  }

  private button(id: string): Button {
    const existing = this.buttons.get(id);
    if (existing) return existing;

    const root = document.createElement("button");
    root.className = "tab";
    root.type = "button";
    const caption = document.createElement("span");
    caption.className = "tab-title";
    const dot = document.createElement("span");
    dot.className = "tab-dirty";
    dot.hidden = true;
    root.append(caption, dot);
    root.addEventListener("mousedown", (event) => event.preventDefault());
    root.addEventListener("pointerdown", (event) => this.drag(id, event));
    root.addEventListener("click", () => this.actions.select(id));

    const button = { root, caption, dot };
    this.buttons.set(id, button);
    this.owners.set(root, id);
    return button;
  }

  private drag(id: string, event: PointerEvent) {
    if (event.button !== 0) return;
    const startX = event.clientX;

    const detach = () => {
      document.removeEventListener("pointermove", move);
      document.removeEventListener("pointerup", up);
      document.removeEventListener("pointercancel", cancel);
      window.removeEventListener("blur", cancel);
    };
    const cancel = () => {
      detach();
      if (!this.dragged) return;
      this.dragged = undefined;
      this.draw();
    };
    const move = (moved: PointerEvent) => {
      if (this.dragged || Math.abs(moved.clientX - startX) <= 4) return;
      this.dragged = id;
      this.draw();
    };
    const up = (released: PointerEvent) => {
      detach();
      if (!this.dragged) return;
      const { pinned, position } = this.dropTarget(id, released.clientX);
      this.dragged = undefined;
      this.draw();
      this.actions.place(id, pinned, position);
    };

    document.addEventListener("pointermove", move);
    document.addEventListener("pointerup", up);
    document.addEventListener("pointercancel", cancel);
    window.addEventListener("blur", cancel);
  }

  private dropTarget(id: string, clientX: number) {
    const pinned = clientX < this.pinnedGroup.getBoundingClientRect().right;
    const group = pinned ? this.pinnedGroup : this.group;
    let position = 0;
    for (const button of group.querySelectorAll(".tab")) {
      if (this.owners.get(button) === id) continue;
      const rect = button.getBoundingClientRect();
      if (clientX > rect.left + rect.width / 2) position++;
    }
    return { pinned, position };
  }
}
