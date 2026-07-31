import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";

/** Serializable argument map accepted by Tauri commands. */
export type InvokeArgs = Record<string, unknown>;

/** Typed facade around the Tauri command bridge. */
export async function invoke<T = void>(command: string, args?: InvokeArgs): Promise<T> {
  return tauriInvoke<T>(command, args);
}

/** Lazily loaded event API, allowing the UI to render in a regular browser. */
export const tauriEvent = import("@tauri-apps/api/event").catch(() => null);

/** Current native window, or null when the application runs outside Tauri. */
export const appWindow = (() => {
  try {
    return getCurrentWindow();
  } catch {
    return null;
  }
})();
