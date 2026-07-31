import { confirm } from "@tauri-apps/plugin-dialog";
import type { ReturnTypeOfProfileActions } from "./internalTypes";
import type { ProfileEditor } from "../../types";
import { CopyButton } from "../../components/ui/CopyButton";
import { Icon } from "../../components/ui/Icon";

const jsonTokenPattern = /"(?:\\.|[^"\\])*"|-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?|\b(?:true|false|null)\b|[{}[\],:]/g;

/** Lightweight JSON highlighting without injecting markup or shipping a parser. */
function HighlightedJson({ content }: { content: string }) {
  const tokens = [];
  let cursor = 0;

  for (const match of content.matchAll(jsonTokenPattern)) {
    const index = match.index;
    if (index > cursor) tokens.push(content.slice(cursor, index));

    const token = match[0];
    const remainder = content.slice(index + token.length);
    const kind = token.startsWith('"')
      ? (/^\s*:/.test(remainder) ? "key" : "string")
      : /^-?\d/.test(token)
        ? "number"
        : /^(true|false|null)$/.test(token)
          ? "literal"
          : "punctuation";
    tokens.push(<span className={`json-${kind}`} key={index}>{token}</span>);
    cursor = index + token.length;
  }

  if (cursor < content.length) tokens.push(content.slice(cursor));
  return <code>{tokens}</code>;
}

/** Profile switcher, persistence actions, and generated client snippets. */
export function ProfileSection({ editor, actions }: { editor: ProfileEditor; actions: ReturnTypeOfProfileActions }) {
  const state = editor.profileState!;
  return (
    <section className="dashboard-card settings-section profile-section">
      <div className="settings-section-title"><h3>Profile</h3><span>Schema v{state.schema_version}</span></div>
      <div className="profile-switcher">
        <div className="profile-select-field">
          <span>Active profile</span>
          <button className="profile-select-control" type="button" aria-haspopup="listbox" aria-expanded={actions.profileMenuOpen} onClick={() => actions.setProfileMenuOpen(!actions.profileMenuOpen)}><span>{editor.profileName}</span><Icon name="expand_more" /></button>
          {actions.profileMenuOpen && <div className="profile-menu" role="listbox" aria-label="Profiles">{state.profiles.map((profile) => <button key={profile.id} type="button" role="option" aria-selected={profile.id === actions.activeId} onClick={() => { actions.setProfileMenuOpen(false); actions.selectProfile(profile.id); }}><span className="profile-menu-icon"><Icon name="folder" /></span><span><strong>{profile.display_name}</strong><small>{profile.roots.length} {profile.roots.length === 1 ? "root" : "roots"}</small></span>{profile.id === actions.activeId && <Icon name="check" />}</button>)}</div>}
        </div>
        <div className="profile-management-actions" aria-label="Profile management">
          <button className="outlined-button" type="button" onClick={() => actions.setProfileDialog({ kind: "create", initialName: "New profile" })}><Icon name="add" /> New</button>
          <button className="outlined-button" type="button" onClick={() => actions.setProfileDialog({ kind: "duplicate", initialName: `${editor.profileName} copy` })}><Icon name="content_copy" /> Duplicate</button>
          <button className="outlined-button" type="button" onClick={() => actions.setProfileDialog({ kind: "rename", initialName: editor.profileName })}><Icon name="edit" /> Rename</button>
          <button className="outlined-button destructive-action" type="button" disabled={state.profiles.length === 1} onClick={async () => { if (await confirm(`Delete profile “${editor.profileName}”?`, { title: "Delete profile", kind: "warning" })) actions.profileAction("delete_profile", { id: actions.activeId }); }}><Icon name="delete" /> Delete</button>
        </div>
      </div>
      <div className="profile-action-label"><span>Configuration</span><small>{editor.dirty ? "Unsaved changes" : "Profile is up to date"}</small></div>
      <div className="configuration-actions">
        <button className="outlined-button save-profile-button" type="button" disabled={!editor.dirty} onClick={() => editor.saveProfile().then((saved) => saved && actions.setActionMessage("Profile saved"))}><Icon name="save" /> Save profile</button>
        <button className="outlined-button" type="button" onClick={actions.exportToml}><Icon name="download" /> Export TOML</button>
        <button className="outlined-button" type="button" onClick={actions.loadSnippets}><Icon name="integration_instructions" /> Client snippets</button>
      </div>
      {actions.snippets.length > 0 && <div className="snippet-list">{actions.snippets.map((snippet) => <details className="snippet-card" key={snippet.id}><summary><span>{snippet.title}</span><Icon name="expand_more" /></summary><pre><HighlightedJson content={snippet.content} /></pre><div className="snippet-actions"><CopyButton value={snippet.content} label={`Copy ${snippet.title}`} /><button type="button" aria-label={`Save ${snippet.title} as JSON`} title="Save as JSON" onClick={() => actions.exportSnippet(snippet)}><Icon name="save_as" /></button></div></details>)}</div>}
    </section>
  );
}
