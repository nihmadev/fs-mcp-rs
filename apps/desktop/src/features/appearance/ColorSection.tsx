import { useEffect, useState } from "react";
import { Icon } from "../../components/ui/Icon";
import type { ThemeController } from "../../types";
import { colorPresets, hexPattern, themeVariables, validateThemeColors } from "./presets";
import type { AppearancePreset, ColorScheme, ThemeColors } from "./types";

const presetLabels: Array<{ id: AppearancePreset; label: string }> = [
  { id: "default", label: "Default" }, { id: "blue", label: "Blue" }, { id: "purple", label: "Purple" },
  { id: "green", label: "Green" }, { id: "orange", label: "Orange" }, { id: "monochrome", label: "Monochrome" },
  { id: "gruvbox", label: "Gruvbox" }, { id: "custom", label: "Custom" },
];
const roles: Array<{ id: keyof ThemeColors; label: string }> = [
  { id: "primary", label: "Primary color" }, { id: "secondary", label: "Secondary color" },
  { id: "accent", label: "Accent color" }, { id: "surface", label: "Surface / background" }, { id: "error", label: "Error color" },
];

export function ColorSection({ theme }: { theme: ThemeController }) {
  const [scheme, setScheme] = useState<ColorScheme>(theme.theme);
  const [draft, setDraft] = useState(theme.settings.colors);
  useEffect(() => setDraft(theme.settings.colors), [theme.settings.colors]);
  const currentError = validateThemeColors(draft[scheme]);
  const otherScheme: ColorScheme = scheme === "light" ? "dark" : "light";
  const otherError = validateThemeColors(draft[otherScheme]);
  const error = currentError || (otherError ? `${otherScheme === "light" ? "Light" : "Dark"} mode: ${otherError}` : "");

  const selectPreset = (preset: AppearancePreset) => {
    if (preset === "custom") {
      theme.updateAppearance({ preset, themeMode: "custom" });
      return;
    }
    theme.updateAppearance({ preset, colors: colorPresets[preset] });
  };
  const setColor = (role: keyof ThemeColors, value: string) => setDraft((current) => ({
    ...current,
    [scheme]: { ...current[scheme], [role]: value.toUpperCase() },
  }));
  const unchangedPreset = JSON.stringify(draft) === JSON.stringify(theme.settings.colors) ? theme.settings.preset : undefined;
  const previewStyle = hexPattern.test(draft[scheme].surface) && Object.values(draft[scheme]).every((value) => hexPattern.test(value))
    ? themeVariables(draft[scheme], scheme, unchangedPreset) as React.CSSProperties : undefined;

  return (
    <>
      <section className="dashboard-card settings-section">
        <div className="settings-section-title"><h3>Material color presets</h3><span>Light and dark</span></div>
        <div className="preset-grid" role="radiogroup" aria-label="Color preset">
          {presetLabels.map((preset) => <button key={preset.id} type="button" role="radio" aria-checked={theme.settings.preset === preset.id} className={`preset-option${theme.settings.preset === preset.id ? " selected" : ""}`} onClick={() => selectPreset(preset.id)}><span className="preset-swatches" aria-hidden="true">{(preset.id === "custom" ? draft.light : colorPresets[preset.id].light) && ["primary", "secondary", "accent"].map((role) => <i key={role} style={{ background: (preset.id === "custom" ? draft.light : colorPresets[preset.id].light)[role as keyof ThemeColors] }} />)}</span><strong>{preset.label}</strong><Icon name={theme.settings.preset === preset.id ? "check_circle" : "circle"} /></button>)}
        </div>
      </section>
      <section className="dashboard-card settings-section">
        <div className="settings-section-title"><h3>Custom colors</h3><span>Preview before applying</span></div>
        <div className="scheme-selector tabs" role="group" aria-label="Color scheme to edit">
          {(["light", "dark"] as const).map((item) => <button key={item} type="button" className={scheme === item ? "active" : ""} aria-pressed={scheme === item} onClick={() => setScheme(item)}><Icon name={item === "light" ? "light_mode" : "dark_mode"} />{item === "light" ? "Light" : "Dark"}</button>)}
        </div>
        <div className="color-editor">
          <div className="color-fields">
            {roles.map((role) => <label key={role.id} className={`color-field${hexPattern.test(draft[scheme][role.id]) ? "" : " invalid"}`}><span>{role.label}</span><span className="color-control"><input type="color" aria-label={`${role.label} picker for ${scheme} mode`} value={hexPattern.test(draft[scheme][role.id]) ? draft[scheme][role.id] : "#000000"} onChange={(event) => setColor(role.id, event.target.value)} /><input type="text" aria-label={`${role.label} HEX value for ${scheme} mode`} value={draft[scheme][role.id]} maxLength={7} spellCheck={false} onChange={(event) => setColor(role.id, event.target.value)} aria-invalid={!hexPattern.test(draft[scheme][role.id])} /></span></label>)}
          </div>
          <div className="appearance-preview" style={previewStyle} data-preview-theme={scheme} aria-label={`${scheme} theme preview`}>
            <div className="preview-rail"><Icon name="palette" /><i /><i /></div>
            <div className="preview-content"><span>Appearance</span><strong>Live preview</strong><small>Material color roles update here first.</small><button type="button" tabIndex={-1}>Primary action</button><div><i /><i /><i /></div></div>
          </div>
        </div>
        {error && <p className="color-error" role="alert"><Icon name="contrast" />{error}</p>}
        <div className="appearance-actions"><button className="outlined-button" type="button" onClick={() => setDraft(theme.settings.colors)}>Discard preview</button><button className="primary-button" type="button" disabled={Boolean(error)} onClick={() => theme.updateAppearance({ colors: draft, preset: "custom", themeMode: theme.settings.themeMode === "custom" ? "custom" : theme.settings.themeMode })}>Apply colors</button></div>
      </section>
    </>
  );
}
