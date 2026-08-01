import type { AdvancedConfig, Permission, Profile } from "../types";

/** Editable fields needed to assemble a persisted profile. */
export type ProfileFormValues = {
  profileName: string;
  roots: string[];
  unrestrictedAccess: boolean;
  port: string;
  selected: Set<Permission>;
  toolLogs: boolean;
  maxReadMb: string;
  searchResults: string;
  includeHidden: boolean;
  advanced: AdvancedConfig;
};

/** Converts persisted profile values into strings suitable for form controls. */
export function advancedConfigFromProfile(profile: Profile): AdvancedConfig {
  return {
    host: profile.host,
    followLinks: profile.follow_links,
    maxWriteMb: String(profile.max_write_mb),
    treeMaxDepth: String(profile.tree_max_depth),
    treeMaxEntries: String(profile.tree_max_entries),
    treeMaxWarnings: String(profile.tree_max_warnings),
    patchMaxKb: String(profile.patch_max_kb),
    patchPreviewKb: String(profile.patch_preview_kb),
    serverConcurrency: String(profile.max_concurrency),
    ioConcurrency: String(profile.max_io_concurrency),
    searchConcurrency: String(profile.search_max_concurrency),
    searchWorkers: String(profile.search_worker_threads),
    regexCacheCapacity: String(profile.regex_cache_capacity),
    respectGitignore: profile.respect_gitignore,
    terminalConcurrency: String(profile.terminal_max_concurrency),
    terminalDefaultTimeoutMs: String(profile.terminal_default_timeout_ms),
    terminalMaxTimeoutMs: String(profile.terminal_max_timeout_ms),
    terminalMaxOutputMb: String(profile.terminal_max_output_mb),
    terminalMaxReadKb: String(profile.terminal_max_read_kb),
    terminalMaxWaitMs: String(profile.terminal_max_wait_ms),
    terminalRetentionMs: String(profile.terminal_session_retention_ms),
    oauthEnabled: profile.oauth_enabled,
    oauthRequireAuth: profile.oauth_require_auth,
    oauthIssuer: profile.oauth_issuer ?? "",
  };
}

/** Derives the enabled permission set from backend profile flags. */
export function permissionsFromProfile(profile: Profile): Set<Permission> {
  return new Set<Permission>([
    "read",
    "search",
    ...(!profile.read_only ? (["write"] as Permission[]) : []),
    ...(profile.terminal_enabled ? (["terminal"] as Permission[]) : []),
  ]);
}

/** Merges current form values into an existing persisted profile. */
export function profileFromForm(stored: Profile, values: ProfileFormValues): Profile {
  const number = (value: string) => Number(value);
  const { advanced } = values;
  return {
    ...stored,
    display_name: values.profileName,
    roots: values.roots,
    unrestricted_access: values.unrestrictedAccess,
    port: number(values.port),
    host: advanced.host,
    max_concurrency: number(advanced.serverConcurrency),
    max_io_concurrency: number(advanced.ioConcurrency),
    read_only: !values.selected.has("write"),
    follow_links: advanced.followLinks,
    max_read_mb: number(values.maxReadMb),
    max_write_mb: number(advanced.maxWriteMb),
    tree_max_depth: number(advanced.treeMaxDepth),
    tree_max_entries: number(advanced.treeMaxEntries),
    tree_max_warnings: number(advanced.treeMaxWarnings),
    patch_max_kb: number(advanced.patchMaxKb),
    patch_preview_kb: number(advanced.patchPreviewKb),
    max_search_results: number(values.searchResults),
    search_max_concurrency: number(advanced.searchConcurrency),
    search_worker_threads: number(advanced.searchWorkers),
    regex_cache_capacity: number(advanced.regexCacheCapacity),
    include_hidden: values.includeHidden,
    respect_gitignore: advanced.respectGitignore,
    terminal_enabled: values.selected.has("terminal"),
    terminal_max_concurrency: number(advanced.terminalConcurrency),
    terminal_default_timeout_ms: number(advanced.terminalDefaultTimeoutMs),
    terminal_max_timeout_ms: number(advanced.terminalMaxTimeoutMs),
    terminal_max_output_mb: number(advanced.terminalMaxOutputMb),
    terminal_max_read_kb: number(advanced.terminalMaxReadKb),
    terminal_max_wait_ms: number(advanced.terminalMaxWaitMs),
    terminal_session_retention_ms: number(advanced.terminalRetentionMs),
    oauth_enabled: advanced.oauthEnabled,
    oauth_require_auth: advanced.oauthRequireAuth,
    oauth_issuer: advanced.oauthIssuer.trim() || null,
    log_tools: values.toolLogs,
  };
}

/** Converts a bind address into a browser-friendly endpoint host. */
export function endpointHost(host: string): string {
  const displayHost = host === "0.0.0.0" || host === "::" ? "127.0.0.1" : host || "127.0.0.1";
  return displayHost.includes(":") ? `[${displayHost}]` : displayHost;
}

/** Splits a command-line argument string while preserving quoted values. */
export function parseTunnelArguments(value: string): string[] {
  return (value.match(/(?:[^\s"]+|"[^"]*")+/g) ?? []).map((argument) => argument.replace(/^"|"$/g, ""));
}
