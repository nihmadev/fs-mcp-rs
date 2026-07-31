import { useState } from "react";
import type { AdvancedConfig } from "../../types";
import { Icon } from "../../components/ui/Icon";
import { GeneralSection, NetworkSection, TunnelSection } from "./GeneralSections";
import { FilesystemSection, SearchSection, TerminalSection } from "./LimitSections";
import { ProfileNameDialog } from "./ProfileNameDialog";
import { ProfileSection } from "./ProfileSection";
import type { SettingsProps } from "./types";
import { useProfileActions } from "./useProfileActions";
import { AdvancedConfiguration } from "./AdvancedConfiguration";

/** Complete profile settings page assembled from capability-focused sections. */
export function DashboardSettings(props: SettingsProps) {
  const { editor } = props;
  const [activeTab, setActiveTab] = useState<"general" | "access" | "limits" | "advanced">("general");
  const actions = useProfileActions(editor, props.discardToml);
  const setValue = <K extends keyof AdvancedConfig>(key: K, value: AdvancedConfig[K]) => {
    editor.setAdvanced((current) => ({ ...current, [key]: value }));
  };

  return (
    <div className="dashboard-page settings-page screen-enter">
      <div className="page-intro"><h2>Agent settings</h2><p>The complete server configuration, grouped by capability.</p></div>
      {props.running && <div className="settings-notice"><Icon name="schedule" /><span>Edits are staged. Restart the server to apply them.</span></div>}
      {actions.actionMessage && <div className="settings-notice" role="status"><Icon name="info" /><span>{actions.actionMessage}</span></div>}
      <div className="tabs settings-tabs" role="tablist" aria-label="Settings categories">
        <button id="settings-tab-general" type="button" role="tab" aria-selected={activeTab === "general"} aria-controls="settings-panel-general" className={activeTab === "general" ? "active" : ""} onClick={() => setActiveTab("general")}><Icon name="tune" /> General</button>
        <button id="settings-tab-access" type="button" role="tab" aria-selected={activeTab === "access"} aria-controls="settings-panel-access" className={activeTab === "access" ? "active" : ""} onClick={() => setActiveTab("access")}><Icon name="admin_panel_settings" /> Access</button>
        <button id="settings-tab-limits" type="button" role="tab" aria-selected={activeTab === "limits"} aria-controls="settings-panel-limits" className={activeTab === "limits" ? "active" : ""} onClick={() => setActiveTab("limits")}><Icon name="speed" /> Limits</button>
        <button id="settings-tab-advanced" type="button" role="tab" aria-selected={activeTab === "advanced"} aria-controls="settings-panel-advanced" className={activeTab === "advanced" ? "active" : ""} onClick={() => setActiveTab("advanced")}><Icon name="code" /> Advanced</button>
      </div>
      {activeTab === "general" && <div id="settings-panel-general" className="settings-tab-panel" role="tabpanel" aria-labelledby="settings-tab-general">
        <ProfileSection editor={editor} actions={actions} discardToml={props.discardToml} />
        <GeneralSection editor={editor} setValue={setValue} />
        <button className="outlined-button setup-again" type="button" onClick={async () => { if (await props.discardToml("open the setup wizard")) props.onOpenSetup(); }}><Icon name="restart_alt" /> Open setup wizard</button>
      </div>}
      {activeTab === "access" && <div id="settings-panel-access" className="settings-tab-panel" role="tabpanel" aria-labelledby="settings-tab-access">
        <TunnelSection provider={props.tunnelProvider} setProvider={props.setTunnelProvider} executable={props.tunnelExecutable} setExecutable={props.setTunnelExecutable} args={props.tunnelArgs} setArgs={props.setTunnelArgs} setTunnel={props.setTunnelOnProfile} />
        <NetworkSection editor={editor} setValue={setValue} />
      </div>}
      {activeTab === "limits" && <div id="settings-panel-limits" className="settings-tab-panel" role="tabpanel" aria-labelledby="settings-tab-limits">
        <FilesystemSection editor={editor} setValue={setValue} />
        <SearchSection editor={editor} setValue={setValue} />
        <TerminalSection editor={editor} setValue={setValue} />
      </div>}
      <div id="settings-panel-advanced" className="settings-tab-panel" role="tabpanel" aria-labelledby="settings-tab-advanced" hidden={activeTab !== "advanced"}>
        <AdvancedConfiguration editor={editor} running={props.running} modified={props.tomlModified} setModified={props.setTomlModified} />
      </div>
      {actions.profileDialog && <ProfileNameDialog kind={actions.profileDialog.kind} initialName={actions.profileDialog.initialName} onCancel={() => actions.setProfileDialog(null)} onSubmit={actions.submitProfileDialog} />}
    </div>
  );
}
