import { useCallback, useState, type CSSProperties } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Icon } from "./components/ui/Icon";
import { Dashboard } from "./features/dashboard/Dashboard";
import { Onboarding } from "./features/onboarding/Onboarding";
import { useProfileEditor } from "./hooks/useProfileEditor";
import { useTheme } from "./hooks/useTheme";
import { invoke } from "./lib/tauri";
import type { ProfileState } from "./types";

/** Root desktop application that selects loading, onboarding, and dashboard modes. */
function App() {
  const editor = useProfileEditor();
  const theme = useTheme();
  const [setupRequested, setSetupRequested] = useState(false);
  const appearanceProps = {
    style: theme.themeVariables as CSSProperties,
    "data-theme": theme.theme,
    "data-density": theme.settings.density,
    "data-text-size": theme.settings.textSize,
    "data-shape": theme.settings.componentShape,
    "data-sidebar-labels": theme.settings.showSidebarLabels,
    "data-motion": theme.settings.reducedMotion || !theme.settings.animations ? "reduced" : "full",
    "data-high-contrast": theme.settings.highContrast,
    "data-visible-focus": theme.settings.visibleFocus,
    "data-large-targets": theme.settings.largeTargets,
  } as const;

  /** Opens a native folder picker and adds the selected root. */
  const browseForRoot = useCallback(async () => {
    const folder = await open({ directory: true, multiple: false });
    if (folder) editor.setRoots((current) => [...current, String(folder)]);
  }, [editor.setRoots]);

  if (editor.loadingProfiles) {
    return <main className="desktop-stage" {...appearanceProps}><div className="startup-state"><Icon name="progress_activity" /><p>Loading profiles...</p></div></main>;
  }

  if (!editor.profileState) {
    const restoreDefaults = () => invoke<ProfileState>("reset_profiles").then((state) => {
      editor.setProfileState(state);
      editor.applyProfile(state.profiles[0]);
    });
    return <main className="desktop-stage" {...appearanceProps}><div className="startup-state"><Icon name="error" /><h1>Profiles could not be loaded</h1><p>{editor.profileError}</p><button className="primary-button" type="button" onClick={restoreDefaults}>Restore defaults</button></div></main>;
  }

  const showOnboarding = setupRequested || !editor.profileState.profiles.find((profile) => profile.id === editor.profileState!.active_profile_id)?.roots.length;
  return (
    <main className="desktop-stage" {...appearanceProps}>
      {showOnboarding
        ? <Onboarding editor={editor} theme={theme} browseForRoot={browseForRoot} onComplete={() => setSetupRequested(false)} />
        : <Dashboard editor={editor} theme={theme} onOpenSetup={() => setSetupRequested(true)} onBrowse={browseForRoot} />}
    </main>
  );
}

export default App;
