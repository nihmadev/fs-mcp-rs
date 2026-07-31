import { tunnelProviders } from "../../constants";
import type { DashboardTab, Permission, TunnelProvider } from "../../types";
import { CopyButton } from "../../components/ui/CopyButton";
import { Icon } from "../../components/ui/Icon";

/** Server status, endpoint, workspace, and remote access dashboard. */
export function OverviewPanel({ running, busy, error, startServer, stopServer, endpoint, roots, selected, navigate, tunnelProvider, activeTunnelProvider, tunnelRunning, tunnelBusy, publicUrl, connectTunnel, disconnectTunnel, restartRequired, restartServer }: {
  running: boolean;
  busy: boolean;
  error: string;
  startServer: () => void;
  stopServer: () => void;
  endpoint: string;
  roots: string[];
  selected: Set<Permission>;
  navigate: (tab: DashboardTab) => void;
  tunnelProvider: TunnelProvider;
  activeTunnelProvider: TunnelProvider | null;
  tunnelRunning: boolean;
  tunnelBusy: boolean;
  publicUrl: string | null;
  connectTunnel: () => void;
  disconnectTunnel: () => void;
  restartRequired: boolean;
  restartServer: () => void;
}) {
  const selectedProvider = tunnelProviders.find((provider) => provider.id === (activeTunnelProvider ?? tunnelProvider))!;
  return (
    <div className="dashboard-page screen-enter">
      <section className={`server-card ${running ? "running" : ""}`}>
        <span className="server-hero-icon"><Icon name={running ? "check_circle" : "dns"} /></span>
        <div className="server-copy"><h2>{running ? "Ready for connections" : "Local MCP server"}</h2><p>{running ? "Clients can connect using the endpoint below." : "Start it when you want to give an AI client access."}</p></div>
        <button className={running && !restartRequired ? "stop-button" : "primary-button"} type="button" disabled={busy} onClick={restartRequired ? restartServer : running ? stopServer : startServer}><Icon name={busy ? "progress_activity" : restartRequired ? "restart_alt" : running ? "stop" : "play_arrow"} /> {busy ? "Please wait" : restartRequired ? "Restart" : running ? "Stop" : "Start"}</button>
      </section>
      {restartRequired && <div className="settings-notice"><Icon name="restart_alt" /><span>Restart required. The running server continues with its previous saved configuration.</span></div>}
      {error && <div className="error-banner" role="alert"><Icon name="error" />{error}</div>}
      <div className="overview-grid">
        <section className="dashboard-card connection-card"><div className="card-heading"><div><h3>Endpoint</h3><p>Streamable HTTP</p></div></div><div className="endpoint-field"><code>{endpoint}</code><CopyButton value={endpoint} label="Copy endpoint" /></div></section>
        <section className="dashboard-card access-card">
          <div className="card-heading"><div><h3>Workspace</h3><p>{roots.length} {roots.length === 1 ? "root" : "roots"}, {selected.size} permissions</p></div></div>
          <div className="access-path"><span className="path-identity"><Icon name="folder" /><span>{roots[0] || "No folder selected"}{roots.length > 1 ? ` +${roots.length - 1} more` : ""}</span></span></div>
          <button className="inline-action" type="button" onClick={() => navigate("access")}>Manage access <Icon name="chevron_right" /></button>
        </section>
        <section className={`dashboard-card remote-card ${tunnelRunning ? "connected" : ""}`}>
          <div className="remote-provider"><span className={`provider-logo ${selectedProvider.id}`}><img src={selectedProvider.icon} alt="" /></span><div><h3>Remote access</h3><p>{tunnelRunning ? `${selectedProvider.name} tunnel is connected` : `Connect securely through ${selectedProvider.name}`}</p></div></div>
          <div className="remote-actions">
            {tunnelRunning && publicUrl && <div className="endpoint-field remote-endpoint"><code>{publicUrl}/mcp</code><CopyButton value={`${publicUrl}/mcp`} label="Copy public endpoint" /></div>}
            {tunnelRunning && !publicUrl && <span className="discovering-url"><Icon name="progress_activity" /> Waiting for public URL</span>}
            <button className={tunnelRunning ? "outlined-button" : "connect-button"} type="button" disabled={tunnelBusy || busy} onClick={tunnelRunning ? disconnectTunnel : connectTunnel}><Icon name={tunnelBusy ? "progress_activity" : tunnelRunning ? "link_off" : "public"} />{tunnelBusy ? "Connecting" : tunnelRunning ? "Disconnect" : "Connect"}</button>
            {!tunnelRunning && <button className="remote-configure" type="button" onClick={() => navigate("settings")}>Configure</button>}
          </div>
        </section>
      </div>
    </div>
  );
}
