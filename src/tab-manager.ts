import { commands, type TabInfo } from "./bindings.ts";
import { reportFailure } from "./report-failure.ts";
import { Tab } from "./tab.ts";
import { TabSelection } from "./tab-selection.ts";
import type { Terminals } from "./terminals.ts";

export interface TabEntry {
  info: TabInfo;
  tab: Tab;
}

export class TabManager {
  private readonly selection = new TabSelection();
  private readonly tabs = new Map<string, Tab>();
  private readonly terminals: Terminals;
  private readonly container: HTMLElement;
  private readonly onChange: () => void;

  constructor(
    terminals: Terminals,
    container: HTMLElement,
    onChange: () => void,
  ) {
    this.terminals = terminals;
    this.container = container;
    this.onChange = onChange;
  }

  get active(): string | undefined {
    return this.selection.active;
  }

  get count(): number {
    return this.selection.items.length;
  }

  find(id: string): TabInfo | undefined {
    return this.selection.find(id);
  }

  entries(): TabEntry[] {
    return this.selection.items.flatMap((info) => {
      const tab = this.tabs.get(info.id);
      return tab ? [{ info, tab }] : [];
    });
  }

  async refresh() {
    this.update(await commands.tabsList());
  }

  update(items: readonly TabInfo[]) {
    const before = this.selection.active;
    this.selection.update(items);
    for (const { id } of items) {
      if (this.tabs.has(id)) continue;
      this.tabs.set(
        id,
        new Tab(id, this.terminals, this.container, this.onChange),
      );
    }
    for (const [id, tab] of this.tabs) {
      if (this.selection.find(id)) continue;
      tab.dispose();
      this.tabs.delete(id);
    }
    this.switchFrom(before);
    this.onChange();
  }

  clear() {
    for (const tab of this.tabs.values()) tab.dispose();
    this.tabs.clear();
    this.selection.clear();
    this.onChange();
  }

  select(id: string | undefined) {
    if (id === undefined) return;
    const before = this.selection.active;
    this.selection.select(id);
    this.switchFrom(before);
    this.onChange();
  }

  selectIndex(index: number) {
    this.select(this.selection.at(index));
  }

  selectLast() {
    this.selectIndex(this.count - 1);
  }

  next() {
    this.select(this.selection.step(1));
  }

  previous() {
    this.select(this.selection.step(-1));
  }

  create() {
    reportFailure(commands.tabNew());
  }

  close(id: string) {
    reportFailure(commands.tabClose(id));
  }

  closeActive() {
    if (this.active) this.close(this.active);
  }

  restart(id: string) {
    reportFailure(commands.tabRestart(id));
  }

  place(id: string, pinned: boolean, position: number | null) {
    reportFailure(commands.tabPlace(id, pinned, position));
  }

  focus() {
    if (this.active) this.tabs.get(this.active)?.focus();
  }

  resume() {
    for (const tab of this.tabs.values()) tab.resume();
  }

  private switchFrom(before: string | undefined) {
    const active = this.selection.active;
    if (active === before) return;
    if (before) this.tabs.get(before)?.hide();
    if (!active) return;
    this.tabs.get(active)?.show();
    reportFailure(commands.tabActivate(active));
  }
}
