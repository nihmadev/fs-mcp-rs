import type { ThemeController } from "../../types";
import { ChoiceGroup } from "./AppearanceControls";
import type { ThemeMode } from "./types";

const themeChoices: Array<{ value: ThemeMode; label: string; description: string; icon: string }> = [
  { value: "system", label: "System", description: "Follow the operating system", icon: "brightness_auto" },
  { value: "light", label: "Light", description: "Use the light color scheme", icon: "light_mode" },
  { value: "dark", label: "Dark", description: "Use the dark color scheme", icon: "dark_mode" },
  { value: "custom", label: "Custom", description: "Use custom colors and system brightness", icon: "palette" },
];

export function ThemeSection({ theme }: { theme: ThemeController }) {
  return (
    <section className="dashboard-card settings-section">
      <div className="settings-section-title"><h3>Theme</h3><span>Applied immediately</span></div>
      <ChoiceGroup label="Application theme" value={theme.settings.themeMode} choices={themeChoices} onChange={theme.setThemeMode} />
    </section>
  );
}
