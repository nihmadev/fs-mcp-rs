import { ToggleRow } from "../../components/ui/FormControls";
import type { ThemeController } from "../../types";

export function AccessibilitySection({ theme }: { theme: ThemeController }) {
  const settings = theme.settings;
  return <section className="dashboard-card settings-section"><div className="settings-section-title"><h3>Accessibility</h3><span>Material contrast preserved</span></div>
    <ToggleRow icon="contrast" title="High contrast mode" description="Strengthen outlines and text separation" checked={settings.highContrast} onChange={(highContrast) => theme.updateAppearance({ highContrast })} />
    <ToggleRow icon="motion_photos_off" title="Reduced motion" description="Disable non-essential movement and transitions" checked={settings.reducedMotion} onChange={(reducedMotion) => theme.updateAppearance({ reducedMotion })} />
    <ToggleRow icon="touch_app" title="Larger interaction targets" description="Increase clickable areas for buttons and controls" checked={settings.largeTargets} onChange={(largeTargets) => theme.updateAppearance({ largeTargets })} />
    <ToggleRow icon="keyboard" title="Visible focus states" description="Keep keyboard focus indicators visible" checked={settings.visibleFocus} onChange={(visibleFocus) => theme.updateAppearance({ visibleFocus })} />
  </section>;
}
