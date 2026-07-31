import type { ProfileEditor } from "../../types";
import { NumberField, ToggleRow } from "../../components/ui/FormControls";
import { Icon } from "../../components/ui/Icon";
import type { SetAdvancedValue } from "./GeneralSections";

/** Filesystem read/write, tree, patch, and symlink limits. */
export function FilesystemSection({ editor, setValue }: { editor: ProfileEditor; setValue: SetAdvancedValue }) {
  const advanced = editor.advanced;
  return (
    <section className="dashboard-card settings-section">
      <div className="settings-section-title"><h3>Filesystem</h3></div>
      <div className="settings-fields two-columns">
        <NumberField label="Max read (MB)" value={editor.maxReadMb} onChange={editor.setMaxReadMb} />
        <NumberField label="Max write (MB)" value={advanced.maxWriteMb} onChange={(value) => setValue("maxWriteMb", value)} />
        <NumberField label="Tree depth" value={advanced.treeMaxDepth} onChange={(value) => setValue("treeMaxDepth", value)} />
        <NumberField label="Tree entries" value={advanced.treeMaxEntries} onChange={(value) => setValue("treeMaxEntries", value)} />
        <NumberField label="Tree warnings" value={advanced.treeMaxWarnings} onChange={(value) => setValue("treeMaxWarnings", value)} />
        <NumberField label="Patch input (KB)" value={advanced.patchMaxKb} onChange={(value) => setValue("patchMaxKb", value)} />
        <NumberField label="Patch preview (KB)" value={advanced.patchPreviewKb} onChange={(value) => setValue("patchPreviewKb", value)} />
      </div>
      <ToggleRow icon="link" title="Follow symbolic links" description="Allow validated paths to traverse symlinks inside the allowed root" checked={advanced.followLinks} onChange={(value) => setValue("followLinks", value)} />
    </section>
  );
}

/** Search traversal and concurrency limits. */
export function SearchSection({ editor, setValue }: { editor: ProfileEditor; setValue: SetAdvancedValue }) {
  const advanced = editor.advanced;
  return (
    <section className="dashboard-card settings-section">
      <div className="settings-section-title"><h3>Search</h3></div>
      <div className="settings-fields two-columns">
        <NumberField label="Result limit" value={editor.searchResults} onChange={editor.setSearchResults} />
        <NumberField label="Concurrent searches" value={advanced.searchConcurrency} onChange={(value) => setValue("searchConcurrency", value)} />
        <NumberField label="Worker threads" value={advanced.searchWorkers} onChange={(value) => setValue("searchWorkers", value)} />
        <NumberField label="Regex cache (0 disables)" value={advanced.regexCacheCapacity} onChange={(value) => setValue("regexCacheCapacity", value)} min={0} />
      </div>
      <ToggleRow icon="visibility" title="Include hidden files" description="Include hidden files and folders in search traversal" checked={editor.includeHidden} onChange={editor.setIncludeHidden} />
      <ToggleRow icon="filter_alt" title="Respect .gitignore" description="Honor Git ignore and exclude files during traversal" checked={advanced.respectGitignore} onChange={(value) => setValue("respectGitignore", value)} />
    </section>
  );
}

/** Terminal session concurrency, timeout, output, and retention limits. */
export function TerminalSection({ editor, setValue }: { editor: ProfileEditor; setValue: SetAdvancedValue }) {
  const advanced = editor.advanced;
  return (
    <section className="dashboard-card settings-section">
      <div className="settings-section-title"><h3>Terminal limits</h3><span>For Run commands</span></div>
      <div className="settings-fields two-columns">
        <NumberField label="Concurrent sessions" value={advanced.terminalConcurrency} onChange={(value) => setValue("terminalConcurrency", value)} />
        <NumberField label="Default timeout (ms)" value={advanced.terminalDefaultTimeoutMs} onChange={(value) => setValue("terminalDefaultTimeoutMs", value)} />
        <NumberField label="Maximum timeout (ms)" value={advanced.terminalMaxTimeoutMs} onChange={(value) => setValue("terminalMaxTimeoutMs", value)} />
        <NumberField label="Output retained (MB)" value={advanced.terminalMaxOutputMb} onChange={(value) => setValue("terminalMaxOutputMb", value)} />
        <NumberField label="Read response (KB)" value={advanced.terminalMaxReadKb} onChange={(value) => setValue("terminalMaxReadKb", value)} />
        <NumberField label="Maximum wait (ms)" value={advanced.terminalMaxWaitMs} onChange={(value) => setValue("terminalMaxWaitMs", value)} />
        <NumberField label="Session retention (ms)" value={advanced.terminalRetentionMs} onChange={(value) => setValue("terminalRetentionMs", value)} />
      </div>
      <div className="inline-warning neutral"><Icon name="terminal" /><span>Commands inherit the desktop app's OS permissions and are not sandboxed by the filesystem root.</span></div>
    </section>
  );
}
