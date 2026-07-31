export type ThemeMode = "system" | "light" | "dark" | "custom";
export type ColorScheme = "light" | "dark";
export type AppearancePreset = "default" | "blue" | "purple" | "green" | "orange" | "monochrome" | "gruvbox" | "custom";
export type InterfaceDensity = "comfortable" | "compact";
export type TextSize = "small" | "default" | "large";
export type ComponentShape = "standard" | "rounded";

export type ThemeColors = {
  primary: string;
  secondary: string;
  accent: string;
  surface: string;
  error: string;
};

export type AppearanceSettings = {
  themeMode: ThemeMode;
  preset: AppearancePreset;
  colors: Record<ColorScheme, ThemeColors>;
  density: InterfaceDensity;
  textSize: TextSize;
  componentShape: ComponentShape;
  showSidebarLabels: boolean;
  animations: boolean;
  highContrast: boolean;
  reducedMotion: boolean;
  largeTargets: boolean;
  visibleFocus: boolean;
};

export type AppearanceTab = "theme" | "colors" | "interface" | "accessibility";
