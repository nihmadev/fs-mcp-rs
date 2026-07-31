import { useEffect, useRef, useState } from "react";
import { flushSync } from "react-dom";
import { colorPresets, defaultAppearanceSettings, themeVariables } from "../features/appearance/presets";
import type { AppearanceSettings, ThemeMode } from "../features/appearance/types";
import type { ThemeController } from "../types";

const STORAGE_KEY = "fs-mcp-rs.appearance";

function loadAppearance(): AppearanceSettings {
  try {
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? "null") as Partial<AppearanceSettings> | null;
    if (!stored) return defaultAppearanceSettings;
    const colors = stored.preset === "gruvbox" ? colorPresets.gruvbox : {
      light: { ...defaultAppearanceSettings.colors.light, ...stored.colors?.light },
      dark: { ...defaultAppearanceSettings.colors.dark, ...stored.colors?.dark },
    };
    return {
      ...defaultAppearanceSettings,
      ...stored,
      colors,
    };
  } catch {
    return defaultAppearanceSettings;
  }
}

/** Manages the application theme and its circular View Transition animation. */
export function useTheme(): ThemeController {
  const [settings, setSettings] = useState(loadAppearance);
  const [systemTheme, setSystemTheme] = useState<"light" | "dark">(() => window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light");
  const themeButtonRef = useRef<HTMLButtonElement>(null);
  const theme = settings.themeMode === "light" || settings.themeMode === "dark" ? settings.themeMode : systemTheme;

  useEffect(() => {
    const query = window.matchMedia("(prefers-color-scheme: dark)");
    const update = () => setSystemTheme(query.matches ? "dark" : "light");
    query.addEventListener("change", update);
    return () => query.removeEventListener("change", update);
  }, []);

  useEffect(() => {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
  }, [settings]);

  const updateAppearance = (patch: Partial<AppearanceSettings>) => setSettings((current) => ({ ...current, ...patch }));
  const setThemeMode = (themeMode: ThemeMode) => updateAppearance({ themeMode });

  const toggleTheme = () => {
    const nextTheme = theme === "light" ? "dark" : "light";
    const rect = themeButtonRef.current?.getBoundingClientRect();
    const x = rect ? rect.left + rect.width / 2 : window.innerWidth;
    const y = rect ? rect.top + rect.height / 2 : 0;
    const radius = Math.hypot(Math.max(x, window.innerWidth - x), Math.max(y, window.innerHeight - y));

    if (!document.startViewTransition || settings.reducedMotion || !settings.animations || window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
      setThemeMode(nextTheme);
      return;
    }

    const transition = document.startViewTransition(() => flushSync(() => setThemeMode(nextTheme)));
    transition.ready.then(() => {
      document.documentElement.animate(
        { clipPath: [`circle(0 at ${x}px ${y}px)`, `circle(${radius}px at ${x}px ${y}px)`] },
        { duration: 500, easing: "cubic-bezier(0.2, 0, 0, 1)", pseudoElement: "::view-transition-new(root)" },
      );
    });
  };

  return {
    theme,
    settings,
    themeVariables: themeVariables(settings.colors[theme], theme, settings.preset),
    themeButtonRef,
    toggleTheme,
    setThemeMode,
    updateAppearance,
    resetAppearance: () => setSettings(defaultAppearanceSettings),
  };
}
