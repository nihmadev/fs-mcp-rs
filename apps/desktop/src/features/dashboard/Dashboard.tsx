import { useEffect, useState } from "react";
import { navigation } from "../../constants";
import { endpointHost } from "../../lib/profile";
import { appWindow } from "../../lib/tauri";
import type { DashboardTab, ProfileEditor, ThemeController, TunnelProvider, TunnelSettings } from "../../types";
import { Icon } from "../../components/ui/Icon";
import { BrandMark } from "../../components/ui/BrandMark";
import { AccessPanel } from "./AccessPanel";
import { ActivityPanel } from "./ActivityPanel";
import { OverviewPanel } from "./OverviewPanel";
import { useDashboardRuntime } from "./useDashboardRuntime";
import { DashboardSettings } from "../settings/DashboardSettings";
import { AppearanceSettings } from "../appearance/AppearanceSettings";
import { confirm } from "@tauri-apps/plugin-dialog";

/** Main application dashboard and navigation shell. */
export function Dashboard({ editor, theme, onOpenSetup, onBrowse }: {
  editor: ProfileEditor;
  theme: ThemeController;
  onOpenSetup: () => void;
  onBrowse: () => void;
}) {
  const [tab, setTab] = useState<DashboardTab>("overview");
  const activeProfile = editor.profileState!.profiles.find((profile) => profile.id === editor.profileState!.active_profile_id)!;
  const [tunnelProvider, setTunnelProvider] = useState<TunnelProvider>(activeProfile.tunnel?.provider ?? "cloudflared");
  const [tunnelExecutable, setTunnelExecutable] = useState(activeProfile.tunnel?.executable ?? "");
  const [tunnelArgs, setTunnelArgs] = useState(activeProfile.tunnel?.extra_args ?? "");
  const [tomlModified, setTomlModified] = useState(false);
  const discardToml = async (action: string) => {
    if (!tomlModified) return true;
    const discard = await confirm(`Discard unsaved TOML changes and ${action}?`, { title: "Unsaved TOML changes", kind: "warning" });
    if (discard) setTomlModified(false);
    return discard;
  };
  const runtime = useDashboardRuntime({ editor, tunnelProvider, tunnelExecutable, tunnelArgs, discardToml });
  const endpoint = `http://${endpointHost(editor.advanced.host)}:${editor.port}/mcp`;

  useEffect(() => {
    const profile = editor.profileState!.profiles.find((item) => item.id === editor.profileState!.active_profile_id);
    setTunnelProvider(profile?.tunnel?.provider ?? "cloudflared");
    setTunnelExecutable(profile?.tunnel?.executable ?? "");
    setTunnelArgs(profile?.tunnel?.extra_args ?? "");
  }, [editor.profileState!.active_profile_id, editor.profileState!.profiles]);

  /** Changes the visible page and updates activity notification state. */
  const navigate = (next: DashboardTab) => {
    runtime.visitTab(next);
    setTab(next);
  };

  /** Stages tunnel settings on the active in-memory profile. */
  const setTunnelOnProfile = (tunnel: TunnelSettings) => editor.setProfileState((state) => state ? ({
    ...state,
    profiles: state.profiles.map((profile) => profile.id === state.active_profile_id ? { ...profile, tunnel } : profile),
  }) : state);

  return (
    <section className="app-window dashboard-window" aria-label="fs-mcp-rs dashboard">
      <div className="dashboard-body">
        <aside className="sidebar">
          <div className="sidebar-brand"><span className="brand-mark"><BrandMark /></span><span className="brand-name">fs-mcp-rs</span></div>
          <nav className="sidebar-nav" aria-label="Dashboard">{navigation.map((item) => <button key={item.id} type="button" className={tab === item.id ? "active" : ""} onClick={() => navigate(item.id)}><span className="rail-indicator"><Icon name={item.icon} /></span><span className="nav-label">{item.label}{item.id === "activity" && runtime.unreadActivity > 0 && <span className="activity-badge" aria-label={`${runtime.unreadActivity} unread tool calls`}>{runtime.unreadActivity > 99 ? "99+" : runtime.unreadActivity}</span>}</span></button>)}</nav>
        </aside>
        <div className="dashboard-shell">
          <header className="dashboard-topbar">
            <div className="topbar-title"><h1>{navigation.find((item) => item.id === tab)?.label}</h1><span>{editor.profileName || "My project"}</span></div>
            <div className="window-controls">
              <button ref={theme.themeButtonRef} type="button" aria-label={`Switch to ${theme.theme === "light" ? "dark" : "light"} theme`} onClick={theme.toggleTheme}><Icon name={theme.theme === "light" ? "dark_mode" : "light_mode"} /></button>
              <button type="button" aria-label="Minimize" onClick={() => appWindow?.minimize()}><Icon name="minimize" /></button>
              <button type="button" aria-label="Maximize" onClick={() => appWindow?.toggleMaximize()}><Icon name="check_box_outline_blank" /></button>
              <button type="button" className="close-btn" aria-label="Close" onClick={async () => { if (await discardToml("close the application")) appWindow?.close(); }}><Icon name="close" /></button>
            </div>
          </header>
          <div className="dashboard-content">
            {tab === "overview" && <OverviewPanel running={runtime.running} busy={runtime.serverBusy} error={runtime.serverError} startServer={runtime.startServer} stopServer={runtime.stopServer} endpoint={endpoint} roots={editor.roots} selected={editor.selected} navigate={navigate} tunnelProvider={tunnelProvider} activeTunnelProvider={runtime.activeTunnelProvider} tunnelRunning={runtime.tunnelRunning} tunnelBusy={runtime.tunnelBusy} publicUrl={runtime.publicUrl} connectTunnel={runtime.connectTunnel} disconnectTunnel={runtime.disconnectTunnel} restartRequired={runtime.running && (editor.dirty || runtime.runtimeProfileId !== editor.profileState!.active_profile_id)} restartServer={runtime.restartServer} />}
            {tab === "access" && <AccessPanel roots={editor.roots} onBrowse={onBrowse} onRemove={(index) => editor.setRoots((items) => items.filter((_, itemIndex) => itemIndex !== index))} selected={editor.selected} togglePermission={editor.togglePermission} />}
            {tab === "activity" && <ActivityPanel logs={runtime.logs} clearLogs={() => runtime.setLogs([])} />}
            <div hidden={tab !== "settings"}><DashboardSettings editor={editor} running={runtime.running} onOpenSetup={onOpenSetup} tunnelProvider={tunnelProvider} setTunnelProvider={setTunnelProvider} tunnelExecutable={tunnelExecutable} setTunnelExecutable={setTunnelExecutable} tunnelArgs={tunnelArgs} setTunnelArgs={setTunnelArgs} setTunnelOnProfile={setTunnelOnProfile} tomlModified={tomlModified} setTomlModified={setTomlModified} discardToml={discardToml} /></div>
            {tab === "appearance" && <AppearanceSettings theme={theme} />}
          </div>
          <nav className="mobile-nav" aria-label="Dashboard">{navigation.map((item) => <button key={item.id} type="button" className={tab === item.id ? "active" : ""} onClick={() => navigate(item.id)}><Icon name={item.icon} /><span>{item.label}</span></button>)}</nav>
        </div>
      </div>
    </section>
  );
}
