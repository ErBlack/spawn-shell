import { events } from "./bindings.ts";
import type { TabManager } from "./tab-manager.ts";

export function listenTabMenu(manager: TabManager) {
  return events.tabMenuChosen.listen(({ payload: chosen }) => {
    switch (chosen.action) {
      case "new":
        manager.create();
        break;
      case "pin":
        manager.place(chosen.tab, chosen.pinned, chosen.pinned ? null : 0);
        break;
      case "restart":
        manager.restart(chosen.tab);
        break;
      case "close":
        manager.close(chosen.tab);
        break;
      default:
        chosen satisfies never;
    }
  });
}
