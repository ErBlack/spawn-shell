import { error as logError } from "@tauri-apps/plugin-log";
import { commands } from "./bindings.ts";

export function showError(message: string) {
  logError(message);
  commands.reportError(message);
}
