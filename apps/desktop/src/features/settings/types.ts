import type { ProfileEditor, TunnelProvider, TunnelSettings } from "../../types";

/** Shared state and actions consumed by all settings sections. */
export type SettingsProps = {
  editor: ProfileEditor;
  running: boolean;
  onOpenSetup: () => void;
  tunnelProvider: TunnelProvider;
  setTunnelProvider: (value: TunnelProvider) => void;
  tunnelExecutable: string;
  setTunnelExecutable: (value: string) => void;
  tunnelArgs: string;
  setTunnelArgs: (value: string) => void;
  setTunnelOnProfile: (tunnel: TunnelSettings) => void;
};
