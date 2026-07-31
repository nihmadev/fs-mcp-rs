import type { ThemeController } from "../../types";
import { appWindow } from "../../lib/tauri";
import { Icon } from "./Icon";

/** Native-style title bar shared by setup and desktop window controls. */
export function TitleBar({ theme, themeButtonRef, toggleTheme, showTheme = true }: ThemeController & { showTheme?: boolean }) {
  return (
    <div className="custom-titlebar">
      <div className="window-controls">
        {showTheme && (
          <button ref={themeButtonRef} type="button" aria-label={`Switch to ${theme === "light" ? "dark" : "light"} theme`} onClick={toggleTheme}>
            <Icon name={theme === "light" ? "dark_mode" : "light_mode"} />
          </button>
        )}
        <button type="button" aria-label="Minimize" onClick={() => appWindow?.minimize()}><Icon name="minimize" /></button>
        <button type="button" aria-label="Maximize" onClick={() => appWindow?.toggleMaximize()}><Icon name="check_box_outline_blank" /></button>
        <button type="button" className="close-btn" aria-label="Close" onClick={() => appWindow?.close()}><Icon name="close" /></button>
      </div>
    </div>
  );
}
