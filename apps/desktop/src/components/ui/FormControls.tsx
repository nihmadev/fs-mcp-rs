import { Icon } from "./Icon";

/** Standard text field used by advanced dashboard settings. */
export function TextField({ label, value, onChange, placeholder, hint }: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  hint?: string;
}) {
  return (
    <label className="config-field">
      <span>{label}</span>
      <input value={value} placeholder={placeholder} onChange={(event) => onChange(event.target.value)} spellCheck={false} />
      {hint && <small>{hint}</small>}
    </label>
  );
}

/** Numeric field that keeps its value as a string for lossless form editing. */
export function NumberField({ label, value, onChange, min = 1, max }: {
  label: string;
  value: string;
  onChange: (value: string) => void;
  min?: number;
  max?: number;
}) {
  return (
    <label className="config-field">
      <span>{label}</span>
      <input type="number" min={min} max={max} step="1" value={value} onChange={(event) => onChange(event.target.value)} />
    </label>
  );
}

/** Accessible settings row for a boolean option. */
export function ToggleRow({ icon, title, description, checked, onChange, tabIndex, disabled = false }: {
  icon: string;
  title: string;
  description: string;
  checked: boolean;
  onChange: (value: boolean) => void;
  tabIndex?: number;
  disabled?: boolean;
}) {
  return (
    <button className="settings-row" type="button" aria-pressed={checked} onClick={() => onChange(!checked)} tabIndex={tabIndex} disabled={disabled}>
      <span className="row-icon"><Icon name={icon} /></span>
      <span className="row-copy"><strong>{title}</strong><small>{description}</small></span>
      <span className={`switch ${checked ? "on" : ""}`}><i /></span>
    </button>
  );
}
