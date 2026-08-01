import { useCallback, useEffect, useState } from "react";
import { defaultAdvancedConfig } from "../constants";
import { invoke } from "../lib/tauri";
import { advancedConfigFromProfile, permissionsFromProfile, profileFromForm } from "../lib/profile";
import type { AdvancedConfig, Permission, Profile, ProfileEditor, ProfileState } from "../types";

/** Owns editable profile fields, persistence, dirty tracking, and initial loading. */
export function useProfileEditor(): ProfileEditor {
  const [profileName, setProfileName] = useState("My project");
  const [roots, setRootsState] = useState<string[]>([]);
  const [unrestrictedAccess, setUnrestrictedAccessState] = useState(false);
  const [port, setPort] = useState("8000");
  const [selected, setSelected] = useState<Set<Permission>>(new Set(["read", "search"]));
  const [toolLogs, setToolLogs] = useState(true);
  const [maxReadMb, setMaxReadMb] = useState("8");
  const [searchResults, setSearchResults] = useState("1000");
  const [includeHidden, setIncludeHidden] = useState(false);
  const [advanced, setAdvanced] = useState<AdvancedConfig>(defaultAdvancedConfig);
  const [profileState, setProfileState] = useState<ProfileState | null>(null);
  const [profileError, setProfileError] = useState("");
  const [loadingProfiles, setLoadingProfiles] = useState(true);
  const [savedSignature, setSavedSignature] = useState("");

  /** Copies a backend profile into all editable form fields. */
  const applyProfile = useCallback((profile: Profile) => {
    setProfileName(profile.display_name);
    setRootsState(profile.roots);
    setUnrestrictedAccessState(profile.unrestricted_access);
    setPort(String(profile.port));
    setToolLogs(profile.log_tools);
    setMaxReadMb(String(profile.max_read_mb));
    setSearchResults(String(profile.max_search_results));
    setIncludeHidden(profile.include_hidden);
    setSelected(permissionsFromProfile(profile));
    setAdvanced(advancedConfigFromProfile(profile));
    setSavedSignature(JSON.stringify(profile));
  }, []);

  useEffect(() => {
    invoke<ProfileState>("load_profiles")
      .then((state) => {
        setProfileState(state);
        applyProfile(state.profiles.find((item) => item.id === state.active_profile_id)!);
      })
      .catch((error) => setProfileError(String(error)))
      .finally(() => setLoadingProfiles(false));
  }, [applyProfile]);

  /** Builds the active backend profile from current form values. */
  const currentProfile = () => {
    const stored = profileState?.profiles.find((item) => item.id === profileState.active_profile_id);
    if (!stored) throw new Error("Active profile is unavailable");
    return profileFromForm(stored, {
      profileName, roots, unrestrictedAccess, port, selected, toolLogs, maxReadMb, searchResults, includeHidden, advanced,
    });
  };

  /** Persists the active profile and updates its dirty baseline. */
  const saveProfile = async () => {
    try {
      const profile = currentProfile();
      const state = await invoke<ProfileState>("save_profile", { profile });
      setProfileState(state);
      setSavedSignature(JSON.stringify(profile));
      setProfileError("");
      return true;
    } catch (error) {
      setProfileError(String(error));
      return false;
    }
  };

  /** Toggles optional permissions while keeping core permissions enabled. */
  const togglePermission = (permission: Permission) => {
    if (permission === "read" || permission === "search") return;
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(permission)) next.delete(permission);
      else next.add(permission);
      return next;
    });
  };

  const setRoots: ProfileEditor["setRoots"] = (value) => {
    setRootsState((current) => {
      const next = typeof value === "function" ? value(current) : value;
      if (next.length > 0) setUnrestrictedAccessState(false);
      return next;
    });
  };

  const setUnrestrictedAccess = (value: boolean) => {
    setUnrestrictedAccessState(value);
    if (value) setRootsState([]);
  };

  let dirty = false;
  if (profileState) dirty = JSON.stringify(currentProfile()) !== savedSignature;

  return {
    profileName, setProfileName, roots, setRoots, unrestrictedAccess, setUnrestrictedAccess, port, setPort, selected, togglePermission,
    toolLogs, setToolLogs, maxReadMb, setMaxReadMb, searchResults, setSearchResults,
    includeHidden, setIncludeHidden, advanced, setAdvanced, profileState, setProfileState,
    profileError, loadingProfiles, applyProfile, currentProfile, saveProfile, dirty,
  };
}
