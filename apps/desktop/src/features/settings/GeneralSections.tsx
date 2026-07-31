import { tunnelProviders } from "../../constants";
import { endpointHost } from "../../lib/profile";
import type { AdvancedConfig, ProfileEditor, TunnelProvider, TunnelSettings } from "../../types";
import { NumberField, TextField, ToggleRow } from "../../components/ui/FormControls";
import { Icon } from "../../components/ui/Icon";

/** General profile name, port, concurrency, and logging controls. */
export function GeneralSection({ editor, setValue }: { editor: ProfileEditor; setValue: SetAdvancedValue }) {
  return (
    <section className="dashboard-card settings-section">
      <div className="settings-section-title"><h3>General</h3></div>
      <div className="settings-fields two-columns">
        <TextField label="Profile name" value={editor.profileName} onChange={editor.setProfileName} />
        <NumberField label="HTTP port" value={editor.port} onChange={editor.setPort} max={65535} />
        <NumberField label="Request concurrency" value={editor.advanced.serverConcurrency} onChange={(value) => setValue("serverConcurrency", value)} />
        <NumberField label="I/O concurrency" value={editor.advanced.ioConcurrency} onChange={(value) => setValue("ioConcurrency", value)} />
      </div>
      <ToggleRow icon="receipt_long" title="Tool logs" description="Write short tool invocation logs to the local process output" checked={editor.toolLogs} onChange={editor.setToolLogs} />
    </section>
  );
}

/** Tunnel provider and command-line configuration. */
export function TunnelSection({ provider, setProvider, executable, setExecutable, args, setArgs, setTunnel }: {
  provider: TunnelProvider;
  setProvider: (value: TunnelProvider) => void;
  executable: string;
  setExecutable: (value: string) => void;
  args: string;
  setArgs: (value: string) => void;
  setTunnel: (value: TunnelSettings) => void;
}) {
  return (
    <section className="dashboard-card settings-section tunnel-settings">
      <div className="settings-section-title"><h3>Remote access</h3><span>Optional</span></div>
      <div className="provider-grid" role="radiogroup" aria-label="Tunnel provider">
        {tunnelProviders.map((item) => <button key={item.id} type="button" role="radio" aria-checked={provider === item.id} className={`provider-option ${provider === item.id ? "selected" : ""}`} onClick={() => { setProvider(item.id); setTunnel({ provider: item.id, executable, extra_args: args }); }}><span className={`provider-logo ${item.id}`}><img src={item.icon} alt="" /></span><span><strong>{item.name}</strong><small>{item.description}</small></span><span className="provider-check"><Icon name="check" /></span></button>)}
      </div>
      <div className="settings-fields two-columns tunnel-fields">
        <TextField label="Executable (optional)" value={executable} onChange={(value) => { setExecutable(value); setTunnel({ provider, executable: value, extra_args: args }); }} placeholder={provider} hint="Leave empty to use the command from PATH." />
        <TextField label="Extra arguments (optional)" value={args} onChange={(value) => { setArgs(value); setTunnel({ provider, executable, extra_args: value }); }} placeholder={provider === "cloudflared" ? "--no-autoupdate" : ""} hint="Separate arguments with spaces; wrap values containing spaces in quotes." />
      </div>
      <div className="inline-warning"><Icon name="warning" /><span>A public URL exposes the permissions and folders granted above. Keep access narrow and disconnect the tunnel when it is no longer needed.</span></div>
      <p className="section-footnote provider-requirements">The provider CLI must already be installed. ngrok may require an authtoken; zrok requires an enabled environment. Cloudflare Quick Tunnels do not require an account.</p>
    </section>
  );
}

/** Bind address and OAuth discovery/authentication controls. */
export function NetworkSection({ editor, setValue }: { editor: ProfileEditor; setValue: SetAdvancedValue }) {
  const remoteBinding = !["127.0.0.1", "::1", "localhost"].includes(editor.advanced.host);
  return (
    <section className="dashboard-card settings-section">
      <div className="settings-section-title"><h3>Network & OAuth</h3><span>{editor.advanced.oauthRequireAuth ? "Protected" : editor.advanced.oauthEnabled ? "Discovery on" : "Off"}</span></div>
      <div className="settings-fields">
        <TextField label="Listen IP address" value={editor.advanced.host} onChange={(value) => editor.setAdvanced((current) => ({ ...current, host: value, oauthRequireAuth: ["127.0.0.1", "::1"].includes(value) ? current.oauthRequireAuth : false }))} hint="Use an IP literal. 127.0.0.1 is local-only; 0.0.0.0 listens on all interfaces." />
        <TextField label="Issuer URL (optional)" value={editor.advanced.oauthIssuer} onChange={(value) => setValue("oauthIssuer", value)} placeholder={`http://${endpointHost(editor.advanced.host)}:${editor.port}`} />
      </div>
      {remoteBinding && <div className="inline-warning"><Icon name="public" /><span>This address may expose tools to other devices. Enable authentication and use a trusted TLS reverse proxy.</span></div>}
      <ToggleRow icon="key" title="OAuth discovery" description="Enable registration, authorization, token, userinfo, metadata, and JWKS endpoints" checked={editor.advanced.oauthEnabled} onChange={(value) => editor.setAdvanced((current) => ({ ...current, oauthEnabled: value, oauthRequireAuth: value ? current.oauthRequireAuth : false }))} />
      <ToggleRow icon="verified_user" title="Require Bearer authentication" description="Local interoperability mode for tokens issued by the built-in OAuth service" checked={editor.advanced.oauthRequireAuth} onChange={(value) => setValue("oauthRequireAuth", value)} disabled={!editor.advanced.oauthEnabled || remoteBinding} />
      {editor.advanced.oauthEnabled && <p className="section-footnote oauth-footnote">OAuth state resets when the server stops. Authentication is restricted to loopback because the built-in provider is intended for local interoperability, not public identity security.</p>}
    </section>
  );
}

/** Type-safe updater shared by advanced settings sections. */
export type SetAdvancedValue = <K extends keyof AdvancedConfig>(key: K, value: AdvancedConfig[K]) => void;
