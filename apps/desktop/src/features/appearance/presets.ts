import type { AppearancePreset, AppearanceSettings, ColorScheme, ThemeColors } from "./types";

export const colorPresets: Record<Exclude<AppearancePreset, "custom">, Record<ColorScheme, ThemeColors>> = {
  default: {
    light: { primary: "#415F91", secondary: "#565F71", accent: "#705575", surface: "#F9F9FF", error: "#BA1A1A" },
    dark: { primary: "#AAC7FF", secondary: "#BEC6DC", accent: "#DDBCE0", surface: "#111318", error: "#FFB4AB" },
  },
  blue: {
    light: { primary: "#0061A4", secondary: "#535F70", accent: "#006874", surface: "#F8F9FF", error: "#BA1A1A" },
    dark: { primary: "#9ECAFF", secondary: "#BBC7DB", accent: "#82D3E0", surface: "#101418", error: "#FFB4AB" },
  },
  purple: {
    light: { primary: "#6750A4", secondary: "#625B71", accent: "#7D5260", surface: "#FFFBFE", error: "#B3261E" },
    dark: { primary: "#D0BCFF", secondary: "#CCC2DC", accent: "#EFB8C8", surface: "#1C1B1F", error: "#F2B8B5" },
  },
  green: {
    light: { primary: "#386A20", secondary: "#55624C", accent: "#006C4C", surface: "#FDFDF5", error: "#BA1A1A" },
    dark: { primary: "#9CD67D", secondary: "#BDCBAF", accent: "#62DBA5", surface: "#11140F", error: "#FFB4AB" },
  },
  orange: {
    light: { primary: "#8B5000", secondary: "#735A2D", accent: "#8F4C38", surface: "#FFF8F4", error: "#BA1A1A" },
    dark: { primary: "#FFB86B", secondary: "#E2C18D", accent: "#FFB5A0", surface: "#17120D", error: "#FFB4AB" },
  },
  monochrome: {
    light: { primary: "#3B3B3B", secondary: "#5F5F5F", accent: "#474747", surface: "#FCF8F8", error: "#BA1A1A" },
    dark: { primary: "#C6C6C6", secondary: "#C6C6C6", accent: "#C6C6C6", surface: "#131313", error: "#FFB4AB" },
  },
  gruvbox: {
    light: { primary: "#79740E", secondary: "#076678", accent: "#AF3A03", surface: "#F9F5D7", error: "#9D0006" },
    dark: { primary: "#B8BB26", secondary: "#83A598", accent: "#FE8019", surface: "#1D2021", error: "#FB4934" },
  },
};

export const defaultAppearanceSettings: AppearanceSettings = {
  themeMode: "system",
  preset: "default",
  colors: colorPresets.default,
  density: "comfortable",
  textSize: "default",
  componentShape: "standard",
  showSidebarLabels: true,
  animations: true,
  highContrast: false,
  reducedMotion: false,
  largeTargets: false,
  visibleFocus: true,
};

export const hexPattern = /^#[0-9A-F]{6}$/i;

function luminance(hex: string) {
  const channels = [1, 3, 5].map((index) => Number.parseInt(hex.slice(index, index + 2), 16) / 255)
    .map((value) => value <= .04045 ? value / 12.92 : ((value + .055) / 1.055) ** 2.4);
  return .2126 * channels[0] + .7152 * channels[1] + .0722 * channels[2];
}

export function contrastRatio(first: string, second: string) {
  const values = [luminance(first), luminance(second)].sort((a, b) => b - a);
  return (values[0] + .05) / (values[1] + .05);
}

export function contrastText(background: string) {
  return contrastRatio(background, "#FFFFFF") >= contrastRatio(background, "#111318") ? "#FFFFFF" : "#111318";
}

export function validateThemeColors(colors: ThemeColors) {
  if (Object.values(colors).some((value) => !hexPattern.test(value))) return "Enter complete six-digit HEX colors.";
  const weakRole = (["primary", "secondary", "accent", "error"] as const)
    .find((role) => contrastRatio(colors[role], colors.surface) < 3);
  if (weakRole) return `${weakRole[0].toUpperCase()}${weakRole.slice(1)} needs at least 3:1 contrast against the surface.`;
  if (contrastRatio(contrastText(colors.surface), colors.surface) < 4.5) return "Surface text needs at least 4.5:1 contrast.";
  return "";
}

function mix(hex: string, target: "#FFFFFF" | "#000000", amount: number) {
  return `#${[1, 3, 5].map((index) => {
    const source = Number.parseInt(hex.slice(index, index + 2), 16);
    const destination = target === "#FFFFFF" ? 255 : 0;
    return Math.round(source + (destination - source) * amount).toString(16).padStart(2, "0");
  }).join("")}`.toUpperCase();
}

export function themeVariables(colors: ThemeColors, scheme: ColorScheme, preset?: AppearancePreset): Record<string, string> {
  if (preset === "gruvbox") {
    return scheme === "dark" ? {
      "--primary": "#B8BB26",
      "--on-primary": "#1D2021",
      "--primary-container": "#3C3836",
      "--on-primary-container": "#B8BB26",
      "--secondary": "#83A598",
      "--secondary-container": "#504945",
      "--on-secondary-container": "#EBDBB2",
      "--accent": "#FE8019",
      "--surface": "#1D2021",
      "--surface-container-lowest": "#1D2021",
      "--surface-container-low": "#282828",
      "--surface-container": "#3C3836",
      "--surface-container-high": "#504945",
      "--surface-container-highest": "#665C54",
      "--on-surface": "#EBDBB2",
      "--on-surface-variant": "#D5C4A1",
      "--outline": "#A89984",
      "--outline-variant": "#665C54",
      "--error": "#FB4934",
      "--error-container": "#3C3836",
      "--on-error-container": "#FB4934",
    } : {
      "--primary": "#79740E",
      "--on-primary": "#F9F5D7",
      "--primary-container": "#EBDBB2",
      "--on-primary-container": "#3C3836",
      "--secondary": "#076678",
      "--secondary-container": "#D5C4A1",
      "--on-secondary-container": "#3C3836",
      "--accent": "#AF3A03",
      "--surface": "#F9F5D7",
      "--surface-container-lowest": "#F9F5D7",
      "--surface-container-low": "#FBF1C7",
      "--surface-container": "#EBDBB2",
      "--surface-container-high": "#D5C4A1",
      "--surface-container-highest": "#BDAE93",
      "--on-surface": "#3C3836",
      "--on-surface-variant": "#504945",
      "--outline": "#7C6F64",
      "--outline-variant": "#BDAE93",
      "--error": "#9D0006",
      "--error-container": "#EBDBB2",
      "--on-error-container": "#9D0006",
    };
  }
  const dark = scheme === "dark";
  const containerTarget = dark ? "#000000" : "#FFFFFF";
  const surfaceTarget = dark ? "#FFFFFF" : "#000000";
  return {
    "--primary": colors.primary,
    "--on-primary": contrastText(colors.primary),
    "--primary-container": mix(colors.primary, containerTarget, dark ? .55 : .78),
    "--on-primary-container": dark ? mix(colors.primary, "#FFFFFF", .72) : mix(colors.primary, "#000000", .42),
    "--secondary": colors.secondary,
    "--secondary-container": mix(colors.secondary, containerTarget, dark ? .58 : .78),
    "--on-secondary-container": dark ? mix(colors.secondary, "#FFFFFF", .72) : mix(colors.secondary, "#000000", .42),
    "--accent": colors.accent,
    "--surface": colors.surface,
    "--surface-container-lowest": mix(colors.surface, surfaceTarget, dark ? .06 : 0),
    "--surface-container-low": mix(colors.surface, surfaceTarget, dark ? .04 : .025),
    "--surface-container": mix(colors.surface, surfaceTarget, dark ? .07 : .05),
    "--surface-container-high": mix(colors.surface, surfaceTarget, dark ? .11 : .075),
    "--surface-container-highest": mix(colors.surface, surfaceTarget, dark ? .15 : .10),
    "--on-surface": contrastText(colors.surface),
    "--on-surface-variant": mix(contrastText(colors.surface), colors.surface === "#FFFFFF" ? "#FFFFFF" : "#000000", .28),
    "--outline": mix(contrastText(colors.surface), colors.surface === "#FFFFFF" ? "#FFFFFF" : "#000000", .52),
    "--outline-variant": mix(contrastText(colors.surface), colors.surface === "#FFFFFF" ? "#FFFFFF" : "#000000", .75),
    "--error": colors.error,
    "--error-container": mix(colors.error, containerTarget, dark ? .52 : .82),
    "--on-error-container": dark ? mix(colors.error, "#FFFFFF", .75) : mix(colors.error, "#000000", .40),
  };
}
