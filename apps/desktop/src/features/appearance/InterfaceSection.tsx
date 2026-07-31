import { ToggleRow } from "../../components/ui/FormControls";
import type { ThemeController } from "../../types";
import { ChoiceGroup } from "./AppearanceControls";

export function InterfaceSection({ theme }: { theme: ThemeController }) {
  const settings = theme.settings;
  return <>
    <section className="dashboard-card settings-section"><div className="settings-section-title"><h3>Layout and type</h3><span>Applied immediately</span></div>
      <div className="interface-options">
        <div><span className="option-label">Interface density</span><ChoiceGroup compact label="Interface density" value={settings.density} choices={[{ value: "comfortable", label: "Comfortable" }, { value: "compact", label: "Compact" }]} onChange={(density) => theme.updateAppearance({ density })} /></div>
        <div><span className="option-label">Text size</span><ChoiceGroup compact label="Text size" value={settings.textSize} choices={[{ value: "small", label: "Small" }, { value: "default", label: "Default" }, { value: "large", label: "Large" }]} onChange={(textSize) => theme.updateAppearance({ textSize })} /></div>
        <div><span className="option-label">Component shape</span><ChoiceGroup compact label="Component shape" value={settings.componentShape} choices={[{ value: "standard", label: "Standard" }, { value: "rounded", label: "Rounded" }]} onChange={(componentShape) => theme.updateAppearance({ componentShape })} /></div>
      </div>
    </section>
    <section className="dashboard-card settings-section"><div className="settings-section-title"><h3>Navigation and motion</h3></div>
      <ToggleRow icon="label" title="Sidebar labels" description="Show text labels below navigation icons" checked={settings.showSidebarLabels} onChange={(showSidebarLabels) => theme.updateAppearance({ showSidebarLabels })} />
      <ToggleRow icon="animation" title="Animations" description="Use Material motion and theme transitions" checked={settings.animations} onChange={(animations) => theme.updateAppearance({ animations })} />
    </section>
  </>;
}
