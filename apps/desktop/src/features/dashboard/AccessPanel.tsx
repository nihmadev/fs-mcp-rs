import { permissions } from "../../constants";
import type { Permission } from "../../types";
import { RootList } from "../../components/access/RootList";
import { ToggleRow } from "../../components/ui/FormControls";

/** Dashboard page for managing allowed roots and client permissions. */
export function AccessPanel({ roots, onBrowse, onRemove, selected, togglePermission }: {
  roots: string[];
  onBrowse: () => void;
  onRemove: (index: number) => void;
  selected: Set<Permission>;
  togglePermission: (permission: Permission) => void;
}) {
  return (
    <div className="dashboard-page narrow-page screen-enter">
      <div className="page-intro"><h2>Workspace access</h2><p>Control exactly where agents can work and which actions they can perform.</p></div>
      <section className="dashboard-card folder-access-card"><div className="card-title-row"><div><h3>Allowed folders</h3><p>All filesystem operations stay inside these roots.</p></div><button type="button" onClick={onBrowse}>Add folder</button></div><RootList roots={roots} onRemove={onRemove} /></section>
      <section className="dashboard-card permission-settings-card">
        <div className="card-title-row"><div><h3>Agent permissions</h3><p>Changes apply the next time the server starts.</p></div></div>
        {permissions.map((permission) => <ToggleRow key={permission.id} icon={permission.icon} title={permission.title} description={permission.description} checked={selected.has(permission.id)} onChange={() => togglePermission(permission.id)} disabled={permission.id === "read" || permission.id === "search"} />)}
      </section>
    </div>
  );
}
