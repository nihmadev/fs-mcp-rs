import { Icon } from "../ui/Icon";

/** Displays allowed filesystem roots and optional remove actions. */
export function RootList({ roots, onRemove }: { roots: string[]; onRemove: (index: number) => void }) {
  return (
    <div className="root-list">
      {roots.length ? roots.map((root, index) => (
        <div className="access-path large root-row" key={`${root}-${index}`}>
          <span className="path-identity"><Icon name="folder_open" /><span title={root}>{root}</span></span>
          <button type="button" aria-label={`Remove folder ${root}`} disabled={roots.length === 1} onClick={() => onRemove(index)}>
            <Icon name="remove_circle_outline" />
          </button>
        </div>
      )) : <div className="empty-roots">No folders selected</div>}
    </div>
  );
}
