import { useState } from "react";
import { confirm, save } from "@tauri-apps/plugin-dialog";
import { invoke, type InvokeArgs } from "../../lib/tauri";
import type { ClientSnippet, ProfileDialogKind, ProfileEditor, ProfileState } from "../../types";

/** Profile management and export operations used by the settings page. */
export function useProfileActions(editor: ProfileEditor) {
  const [actionMessage, setActionMessage] = useState("");
  const [snippets, setSnippets] = useState<ClientSnippet[]>([]);
  const [profileMenuOpen, setProfileMenuOpen] = useState(false);
  const [profileDialog, setProfileDialog] = useState<{ kind: ProfileDialogKind; initialName: string } | null>(null);
  const activeId = editor.profileState!.active_profile_id;

  /** Switches profiles after confirming the loss of unsaved edits. */
  const selectProfile = async (id: string) => {
    if (editor.dirty && !await confirm("Discard unsaved changes and switch profiles?", { title: "Switch profile", kind: "warning" })) return;
    try {
      const state = await invoke<ProfileState>("set_active_profile", { id });
      editor.setProfileState(state);
      editor.applyProfile(state.profiles.find((profile) => profile.id === id)!);
      setSnippets([]);
      setActionMessage("");
    } catch (error) {
      setActionMessage(String(error));
    }
  };

  /** Executes a profile mutation and applies the backend's active profile. */
  const profileAction = async (command: string, args: InvokeArgs) => {
    try {
      const state = await invoke<ProfileState>(command, args);
      editor.setProfileState(state);
      editor.applyProfile(state.profiles.find((item) => item.id === state.active_profile_id)!);
      setActionMessage("");
      return null;
    } catch (error) {
      const message = String(error);
      setActionMessage(message);
      return message;
    }
  };

  /** Submits create, duplicate, or rename from the shared name dialog. */
  const submitProfileDialog = (name: string) => {
    if (!profileDialog) return Promise.resolve(null);
    if (profileDialog.kind === "rename") return profileAction("rename_profile", { id: activeId, name });
    return profileAction("create_profile", { name, duplicateId: profileDialog.kind === "duplicate" ? activeId : null });
  };

  /** Exports the current saved profile as TOML. */
  const exportToml = async () => {
    try {
      if (editor.dirty && !await editor.saveProfile()) return;
      const filename = editor.profileName.replace(/[<>:"/\\|?*]+/g, "-") || "profile";
      const path = await save({ defaultPath: `${filename}.toml`, filters: [{ name: "TOML configuration", extensions: ["toml"] }] });
      if (!path) return;
      try {
        await invoke("export_profile_toml", { profileId: activeId, path, overwrite: false });
      } catch (error) {
        if (!String(error).includes("already exists") || !await confirm("This file already exists. Replace it?", { title: "Export TOML", kind: "warning" })) throw error;
        await invoke("export_profile_toml", { profileId: activeId, path, overwrite: true });
      }
      setActionMessage(`Configuration saved to ${path}`);
    } catch (error) {
      setActionMessage(String(error));
    }
  };

  /** Loads generated MCP client snippets for the active profile. */
  const loadSnippets = async () => {
    try {
      if (editor.dirty && !await editor.saveProfile()) return;
      setSnippets(await invoke<ClientSnippet[]>("get_client_snippets", { profileId: activeId }));
      setActionMessage("");
    } catch (error) {
      setActionMessage(String(error));
    }
  };

  /** Saves one generated client snippet as JSON. */
  const exportSnippet = async (snippet: ClientSnippet) => {
    const path = await save({ defaultPath: `${snippet.id}.json`, filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!path) return;
    try {
      await invoke("save_snippet", { path, content: snippet.content, overwrite: false });
      setActionMessage(`Snippet saved to ${path}`);
    } catch (error) {
      if (String(error).includes("already exists") && await confirm("This file already exists. Replace it?", { title: "Save snippet", kind: "warning" })) {
        await invoke("save_snippet", { path, content: snippet.content, overwrite: true });
        setActionMessage(`Snippet saved to ${path}`);
      } else setActionMessage(String(error));
    }
  };

  return {
    actionMessage, setActionMessage, snippets, profileMenuOpen, setProfileMenuOpen,
    profileDialog, setProfileDialog, activeId, selectProfile, profileAction,
    submitProfileDialog, exportToml, loadSnippets, exportSnippet,
  };
}
