import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export async function setupWindow(
  pane: HTMLElement,
  hooks: {
    fitAndResize(): void;
    focus(): void;
    ensureAlive(): void;
  },
): Promise<void> {
  const slide = async (toVisible: boolean): Promise<boolean> => {
    const keyframes = toVisible
      ? [{ transform: "translateY(-100%)" }, { transform: "translateY(0)" }]
      : [{ transform: "translateY(0)" }, { transform: "translateY(-100%)" }];
    for (const running of document.body.getAnimations()) running.cancel();
    const animation = document.body.animate(keyframes, {
      duration: 180,
      easing: "linear",
      fill: "forwards",
    });
    try {
      await animation.finished;
      animation.commitStyles();
      animation.cancel();
      return true;
    } catch {
      return false;
    }
  };

  const reveal = () => {
    requestAnimationFrame(() => {
      slide(true);
      hooks.fitAndResize();
      hooks.ensureAlive();
      hooks.focus();
    });
  };

  new ResizeObserver(() => hooks.fitAndResize()).observe(pane);

  await Promise.all([
    listen("summon://show", reveal),
    listen("summon://hide", async () => {
      if (await slide(false)) invoke("hide_window");
    }),
  ]);

  if (await invoke<boolean>("is_open")) reveal();
  else await invoke("hide_window");
}
