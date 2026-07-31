import { useState } from "react";
import { permissions } from "../../constants";
import type { AccessTab, ProfileEditor, ThemeController } from "../../types";
import { RootList } from "../../components/access/RootList";
import { Icon } from "../../components/ui/Icon";
import { BrandMark } from "../../components/ui/BrandMark";
import { TitleBar } from "../../components/ui/TitleBar";
import { ToggleRow } from "../../components/ui/FormControls";

/** Props required by the three-step onboarding wizard. */
type OnboardingProps = {
  editor: ProfileEditor;
  theme: ThemeController;
  onComplete: () => void;
  browseForRoot: () => void;
};

/** Guides first-time users through roots, permissions, and basic profile settings. */
export function Onboarding({ editor, theme, onComplete, browseForRoot }: OnboardingProps) {
  const [step, setStep] = useState(0);
  const [accessTab, setAccessTab] = useState<AccessTab>("folder");
  const [advancedOpen, setAdvancedOpen] = useState(false);

  const continueFlow = () => {
    if (step === 0) return setStep(1);
    if (step === 1 && accessTab === "folder") return setAccessTab("permissions");
    if (step === 2) {
      editor.saveProfile().then((saved) => saved && onComplete());
      return;
    }
    setStep((current) => Math.min(2, current + 1));
  };

  const goBack = () => {
    if (step === 1 && accessTab === "permissions") return setAccessTab("folder");
    setStep((current) => Math.max(0, current - 1));
  };

  return (
    <section className="app-window" aria-label="fs-mcp-rs setup">
      <TitleBar {...theme} />
      <div className="progress" aria-label={`Step ${step + 1} of 3`}>
        {[0, 1, 2].map((item) => <span key={item} className={item <= step ? "active" : ""} />)}
      </div>

      {step === 0 && <WelcomeStep />}
      {step === 1 && (
        <AccessStep
          accessTab={accessTab}
          setAccessTab={setAccessTab}
          editor={editor}
          browseForRoot={browseForRoot}
        />
      )}
      {step === 2 && <FinishStep editor={editor} advancedOpen={advancedOpen} setAdvancedOpen={setAdvancedOpen} />}

      <footer className="footer-actions">
        <button className="text-button" type="button" onClick={goBack} disabled={step === 0}>Back</button>
        <span>Step {step + 1} of 3</span>
        <button className="primary-button" type="button" onClick={continueFlow} disabled={step === 1 && accessTab === "folder" && !editor.roots.length}>
          {step === 0 ? "Get started" : step === 2 ? "Create profile" : "Continue"}
        </button>
      </footer>
    </section>
  );
}

/** Introductory onboarding screen. */
function WelcomeStep() {
  return (
    <section className="welcome screen-enter">
      <div className="welcome-heading">
        <span className="welcome-icon"><BrandMark /></span>
        <h1>Welcome to fs-mcp-rs</h1>
      </div>
      <p className="supporting-text">Choose which folder your AI client can access and set clear permissions. Everything runs locally on this device.</p>
      <div className="local-note"><Icon name="lock" /><span>No cloud account or upload required</span></div>
    </section>
  );
}

/** Folder and permission selection screen. */
function AccessStep({ accessTab, setAccessTab, editor, browseForRoot }: {
  accessTab: AccessTab;
  setAccessTab: (tab: AccessTab) => void;
  editor: ProfileEditor;
  browseForRoot: () => void;
}) {
  return (
    <section className="setup-screen screen-enter">
      <div className="screen-heading"><h1>Set up agent access</h1><p>Select a folder and choose the actions available to your AI client.</p></div>
      <div className="tabs" role="tablist" aria-label="Access setup">
        <button type="button" role="tab" aria-selected={accessTab === "folder"} className={accessTab === "folder" ? "active" : ""} onClick={() => setAccessTab("folder")}><Icon name="folder" /> Folder</button>
        <button type="button" role="tab" aria-selected={accessTab === "permissions"} className={accessTab === "permissions" ? "active" : ""} onClick={() => setAccessTab("permissions")}><Icon name="admin_panel_settings" /> Permissions<span className="count-badge">{editor.selected.size}</span></button>
      </div>
      <div className="tab-panel" key={accessTab}>
        {accessTab === "folder" ? (
          <div className="folder-panel">
            <label className="field-label">Allowed folders</label>
            <RootList roots={editor.roots} onRemove={(index) => editor.setRoots((items) => items.filter((_, itemIndex) => itemIndex !== index))} />
            <button className="outlined-button add-folder-button" type="button" onClick={browseForRoot}><Icon name="create_new_folder" /> Add folder</button>
            <p className="field-help"><Icon name="info" /> The agent cannot access files outside this folder.</p>
          </div>
        ) : (
          <div className="permissions-panel">
            <div className="permission-list">{permissions.map((permission) => {
              const checked = editor.selected.has(permission.id);
              return <button type="button" className="permission-row" aria-pressed={checked} disabled={permission.id === "read" || permission.id === "search"} key={permission.id} onClick={() => editor.togglePermission(permission.id)}><span className="row-icon"><Icon name={permission.icon} /></span><span className="row-copy"><strong>{permission.title}</strong><small>{permission.description}</small></span><span className="elevated">{permission.elevated ? "Elevated" : "Core"}</span><span className={`checkbox ${checked ? "checked" : ""}`}>{checked && <Icon name="check" />}</span></button>;
            })}</div>
            {editor.selected.has("terminal") && <div className="warning-note"><Icon name="warning" /><span>Commands run with your local user permissions.</span></div>}
          </div>
        )}
      </div>
    </section>
  );
}

/** Basic profile settings shown before completing onboarding. */
function FinishStep({ editor, advancedOpen, setAdvancedOpen }: { editor: ProfileEditor; advancedOpen: boolean; setAdvancedOpen: (value: boolean) => void }) {
  const setOauthEnabled = (value: boolean) => editor.setAdvanced((current) => ({ ...current, oauthEnabled: value, oauthRequireAuth: value ? current.oauthRequireAuth : false }));
  return (
    <section className="settings-screen screen-enter">
      <div className="screen-heading"><h1>Finish setup</h1><p>These defaults are recommended for a local MCP connection.</p></div>
      <div className="settings-form">
        <div className="field-grid">
          <label className="outlined-field"><span>Profile name</span><input value={editor.profileName} onChange={(event) => editor.setProfileName(event.target.value)} /></label>
          <label className="outlined-field port-field"><span>HTTP port</span><input value={editor.port} onChange={(event) => editor.setPort(event.target.value)} inputMode="numeric" /></label>
        </div>
        <div className="settings-list">
          <ToggleRow icon="receipt_long" title="Activity logs" description="Write short tool invocation logs to the local process output" checked={editor.toolLogs} onChange={editor.setToolLogs} />
          <button className={`settings-row advanced-row ${advancedOpen ? "open" : ""}`} type="button" aria-expanded={advancedOpen} aria-controls="advanced-settings" onClick={() => setAdvancedOpen(!advancedOpen)}><span className="row-icon"><Icon name="tune" /></span><span className="row-copy"><strong>Advanced settings</strong><small>File and search limits</small></span><Icon name="expand_more" /></button>
        </div>
        <div id="advanced-settings" className={`advanced-panel ${advancedOpen ? "open" : ""}`} aria-hidden={!advancedOpen}>
          <div className="advanced-panel-inner">
            <div className="advanced-grid">
              <label className="outlined-field"><span>Max read size (MB)</span><input value={editor.maxReadMb} onChange={(event) => editor.setMaxReadMb(event.target.value)} inputMode="numeric" tabIndex={advancedOpen ? 0 : -1} /></label>
              <label className="outlined-field"><span>Search result limit</span><input value={editor.searchResults} onChange={(event) => editor.setSearchResults(event.target.value)} inputMode="numeric" tabIndex={advancedOpen ? 0 : -1} /></label>
            </div>
            <ToggleRow icon="visibility" title="Include hidden files" description="Allow search to include hidden files and folders" checked={editor.includeHidden} onChange={editor.setIncludeHidden} tabIndex={advancedOpen ? 0 : -1} />
            <ToggleRow icon="key" title="OAuth discovery" description="Expose OAuth 2.0 and OpenID Connect endpoints" checked={editor.advanced.oauthEnabled} onChange={setOauthEnabled} tabIndex={advancedOpen ? 0 : -1} />
            {editor.advanced.oauthEnabled && <ToggleRow icon="verified_user" title="Require authentication" description="Reject /mcp requests without a valid Bearer token" checked={editor.advanced.oauthRequireAuth} onChange={(value) => editor.setAdvanced((current) => ({ ...current, oauthRequireAuth: value }))} tabIndex={advancedOpen ? 0 : -1} />}
          </div>
        </div>
      </div>
    </section>
  );
}
