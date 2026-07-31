import { useEffect, useState } from "react";
import { confirm, open, save } from "@tauri-apps/plugin-dialog";
import { Icon } from "../../components/ui/Icon";
import { invoke } from "../../lib/tauri";
import type { ProfileEditor, ProfileState } from "../../types";

type EditorState = "Clean" | "Modified" | "Valid" | "Invalid";
type AppliedToml = { state: ProfileState; toml: string };

/** Advanced editor for the active profile's canonical server TOML. */
export function AdvancedConfiguration({ editor, running, modified, setModified }: {
  editor: ProfileEditor;
  running: boolean;
  modified: boolean;
  setModified: (value: boolean) => void;
}) {
  const [text, setText] = useState("");
  const [baseline, setBaseline] = useState("");
  const [state, setState] = useState<EditorState>("Clean");
  const [message, setMessage] = useState("");
  const [sourcePath, setSourcePath] = useState<string | null>(null);
  const [busy, setBusy] = useState<"load" | "validate" | "apply" | "import" | "export" | "">("");
  const profileSignature = JSON.stringify(editor.currentProfile());
  const activeId = editor.profileState!.active_profile_id;

  useEffect(() => {
    if (modified) return;
    let cancelled = false;
    setBusy("load");
    invoke<string>("get_profile_toml", { profile: editor.currentProfile() })
      .then((value) => {
        if (cancelled) return;
        setText(value);
        setBaseline(value);
        setState("Clean");
        setSourcePath(null);
        setMessage("");
      })
      .catch((error) => !cancelled && setMessage(String(error)))
      .finally(() => !cancelled && setBusy(""));
    return () => { cancelled = true; };
  }, [activeId, profileSignature, modified]);

  const updateText = (value: string) => {
    setText(value);
    const changed = value !== baseline;
    setModified(changed);
    setState(changed ? "Modified" : "Clean");
    setMessage("");
  };

  const validate = async () => {
    setBusy("validate");
    try {
      await invoke<string>("validate_profile_toml", { profileId: activeId, toml: text, sourcePath });
      setState("Valid");
      setMessage("Configuration is valid. No profile changes were saved.");
    } catch (error) {
      setState("Invalid");
      setMessage(String(error));
    } finally {
      setBusy("");
    }
  };

  const apply = async () => {
    setBusy("apply");
    try {
      const result = await invoke<AppliedToml>("apply_profile_toml", { profileId: activeId, toml: text, sourcePath });
      const profile = result.state.profiles.find((item) => item.id === activeId)!;
      editor.setProfileState(result.state);
      editor.applyProfile(profile);
      setText(result.toml);
      setBaseline(result.toml);
      setModified(false);
      setState("Clean");
      setSourcePath(null);
      setMessage(running ? "Profile saved. Restart the server to use this configuration." : "Profile saved and synchronized with the GUI.");
    } catch (error) {
      setState("Invalid");
      setMessage(String(error));
    } finally {
      setBusy("");
    }
  };

  const reset = async () => {
    if (modified && !await confirm("Discard the unsaved TOML changes?", { title: "Reset configuration", kind: "warning" })) return;
    setModified(false);
    setText(baseline);
    setState("Clean");
    setSourcePath(null);
    setMessage("");
  };

  const importToml = async () => {
    if (modified && !await confirm("Replace the unsaved TOML changes with a file?", { title: "Import TOML", kind: "warning" })) return;
    const path = await open({ multiple: false, directory: false, filters: [{ name: "TOML configuration", extensions: ["toml"] }] });
    if (!path) return;
    setBusy("import");
    try {
      const value = await invoke<string>("read_toml_file", { path });
      setText(value);
      setSourcePath(path);
      setModified(value !== baseline);
      setState(value === baseline ? "Clean" : "Modified");
      setMessage(`Imported ${path}`);
    } catch (error) {
      setState("Invalid");
      setMessage(String(error));
    } finally {
      setBusy("");
    }
  };

  const exportToml = async () => {
    const path = await save({ defaultPath: `${editor.profileName.replace(/[<>:"/\\|?*]+/g, "-") || "profile"}.toml`, filters: [{ name: "TOML configuration", extensions: ["toml"] }] });
    if (!path) return;
    setBusy("export");
    try {
      await invoke("save_text_file", { path, content: text, overwrite: false });
      setMessage(`Editor contents saved to ${path}`);
    } catch (error) {
      if (!String(error).includes("already exists") || !await confirm("This file already exists. Replace it?", { title: "Export TOML", kind: "warning" })) {
        setMessage(String(error));
        setBusy("");
        return;
      }
      await invoke("save_text_file", { path, content: text, overwrite: true });
      setMessage(`Editor contents saved to ${path}`);
    } finally {
      setBusy("");
    }
  };

  return (
    <section className="dashboard-card settings-section advanced-configuration">
      <div className="settings-section-title"><h3>Profile configuration</h3><span className={`toml-state state-${state.toLowerCase()}`}>{state}</span></div>
      <div className="toml-editor-copy"><h3>Server TOML</h3><p>Edit the same active profile exposed by the visual controls. Validation and security checks run in Rust.</p></div>
      {running && <div className="inline-warning neutral"><Icon name="restart_alt" /><span>Applied changes are staged until you restart the running server.</span></div>}
      <label className="toml-editor-field" htmlFor="profile-toml"><span>Configuration TOML</span><textarea id="profile-toml" value={text} disabled={busy === "load"} onChange={(event) => updateText(event.target.value)} spellCheck={false} aria-describedby="toml-editor-status" aria-invalid={state === "Invalid"} /></label>
      <div className="toml-actions">
        <button className="outlined-button" type="button" disabled={Boolean(busy)} onClick={importToml}><Icon name="upload_file" />{busy === "import" ? "Importing..." : "Import"}</button>
        <button className="outlined-button" type="button" disabled={Boolean(busy) || !text} onClick={exportToml}><Icon name="download" />{busy === "export" ? "Exporting..." : "Export"}</button>
        <span />
        <button className="outlined-button" type="button" disabled={Boolean(busy) || !modified} onClick={reset}><Icon name="restart_alt" /> Reset</button>
        <button className="outlined-button" type="button" disabled={Boolean(busy) || !text} onClick={validate}><Icon name="fact_check" />{busy === "validate" ? "Validating..." : "Validate"}</button>
        <button className="primary-button toml-apply" type="button" disabled={Boolean(busy) || !modified || !text} onClick={apply}><Icon name={busy === "apply" ? "progress_activity" : "check"} />{busy === "apply" ? "Applying..." : "Apply"}</button>
      </div>
      {message && <div id="toml-editor-status" className={state === "Invalid" ? "toml-result invalid" : "toml-result"} role={state === "Invalid" ? "alert" : "status"}><Icon name={state === "Invalid" ? "error" : "info"} /><span>{message}</span></div>}
    </section>
  );
}
