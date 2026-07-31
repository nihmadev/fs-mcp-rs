import cloudflareIcon from "./assets/cloudflare.svg";
import ngrokIcon from "./assets/ngrok.svg";
import zrokIcon from "./assets/zrok.svg";
import type { AdvancedConfig, DashboardTab, Permission, TunnelProvider } from "./types";

/** Default form values used until a persisted profile is loaded. */
export const defaultAdvancedConfig: AdvancedConfig = {
  host: "127.0.0.1",
  followLinks: false,
  maxWriteMb: "8",
  treeMaxDepth: "8",
  treeMaxEntries: "1000",
  treeMaxWarnings: "32",
  patchMaxKb: "1024",
  patchPreviewKb: "16",
  serverConcurrency: "32",
  ioConcurrency: "16",
  searchConcurrency: "4",
  searchWorkers: "4",
  regexCacheCapacity: "64",
  respectGitignore: true,
  terminalConcurrency: "2",
  terminalDefaultTimeoutMs: "30000",
  terminalMaxTimeoutMs: "300000",
  terminalMaxOutputMb: "4",
  terminalMaxReadKb: "256",
  terminalMaxWaitMs: "30000",
  terminalRetentionMs: "300000",
  oauthEnabled: true,
  oauthRequireAuth: false,
  oauthIssuer: "",
};

/** Metadata used to render and explain each permission. */
export const permissions: Array<{
  id: Permission;
  icon: string;
  title: string;
  description: string;
  elevated?: boolean;
}> = [
  { id: "read", icon: "visibility", title: "Read files", description: "View files and folders" },
  { id: "search", icon: "search", title: "Search", description: "Find files and text" },
  { id: "write", icon: "edit", title: "Modify files", description: "Create, edit, move, and delete", elevated: true },
  { id: "terminal", icon: "terminal", title: "Run commands", description: "Execute local shell commands", elevated: true },
];

/** Sidebar and mobile navigation entries. */
export const navigation: Array<{ id: DashboardTab; icon: string; label: string }> = [
  { id: "overview", icon: "space_dashboard", label: "Overview" },
  { id: "access", icon: "admin_panel_settings", label: "Access" },
  { id: "activity", icon: "receipt_long", label: "Activity" },
  { id: "settings", icon: "settings", label: "Settings" },
  { id: "appearance", icon: "palette", label: "Appearance" },
];

/** User-facing metadata and logos for supported tunnel providers. */
export const tunnelProviders: Array<{
  id: TunnelProvider;
  name: string;
  description: string;
  icon: string;
}> = [
  { id: "cloudflared", name: "Cloudflare", description: "Quick Tunnel, no account required", icon: cloudflareIcon },
  { id: "ngrok", name: "ngrok", description: "Uses your configured ngrok account", icon: ngrokIcon },
  { id: "zrok", name: "zrok", description: "Public share via OpenZiti", icon: zrokIcon },
];
