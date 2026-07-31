import { useRef, useState, type KeyboardEvent } from "react";
import { confirm } from "@tauri-apps/plugin-dialog";
import { Icon } from "../../components/ui/Icon";
import type { ThemeController } from "../../types";
import { AccessibilitySection } from "./AccessibilitySection";
import { ColorSection } from "./ColorSection";
import { InterfaceSection } from "./InterfaceSection";
import { ThemeSection } from "./ThemeSection";
import type { AppearanceTab } from "./types";

const tabs: Array<{ id: AppearanceTab; label: string; icon: string }> = [
  { id: "theme", label: "Theme", icon: "brightness_6" }, { id: "colors", label: "Colors", icon: "palette" },
  { id: "interface", label: "Interface", icon: "dashboard_customize" }, { id: "accessibility", label: "Accessibility", icon: "accessibility_new" },
];

export function AppearanceSettings({ theme }: { theme: ThemeController }) {
  const [activeTab, setActiveTab] = useState<AppearanceTab>("theme");
  const tablistRef = useRef<HTMLDivElement>(null);
  const selectFromKeyboard = (event: KeyboardEvent<HTMLDivElement>) => {
    const current = tabs.findIndex((tab) => tab.id === activeTab);
    let next = current;
    if (event.key === "ArrowRight" || event.key === "ArrowDown") next = (current + 1) % tabs.length;
    else if (event.key === "ArrowLeft" || event.key === "ArrowUp") next = (current - 1 + tabs.length) % tabs.length;
    else if (event.key === "Home") next = 0;
    else if (event.key === "End") next = tabs.length - 1;
    else return;
    event.preventDefault();
    setActiveTab(tabs[next].id);
    tablistRef.current?.querySelectorAll<HTMLButtonElement>("[role=tab]")[next]?.focus();
  };
  const reset = async () => {
    if (await confirm("Reset all appearance settings to their defaults?", { title: "Reset appearance", kind: "warning" })) theme.resetAppearance();
  };

  return <div className="dashboard-page settings-page appearance-page screen-enter">
    <div className="page-intro page-intro-actions"><div><h2>Appearance</h2><p>Customize the application theme, colors, interface, and accessibility.</p></div><button className="outlined-button destructive-action" type="button" onClick={reset}><Icon name="restart_alt" />Reset appearance</button></div>
    <div ref={tablistRef} className="tabs settings-tabs appearance-tabs" role="tablist" aria-label="Appearance categories" onKeyDown={selectFromKeyboard}>
      {tabs.map((tab) => <button key={tab.id} id={`appearance-tab-${tab.id}`} type="button" role="tab" tabIndex={activeTab === tab.id ? 0 : -1} aria-selected={activeTab === tab.id} aria-controls={`appearance-panel-${tab.id}`} className={activeTab === tab.id ? "active" : ""} onClick={() => setActiveTab(tab.id)}><Icon name={tab.icon} /><span>{tab.label}</span></button>)}
    </div>
    <div id={`appearance-panel-${activeTab}`} className="settings-tab-panel" role="tabpanel" tabIndex={0} aria-labelledby={`appearance-tab-${activeTab}`}>
      {activeTab === "theme" && <ThemeSection theme={theme} />}
      {activeTab === "colors" && <ColorSection theme={theme} />}
      {activeTab === "interface" && <InterfaceSection theme={theme} />}
      {activeTab === "accessibility" && <AccessibilitySection theme={theme} />}
    </div>
  </div>;
}
