import { Icon } from "../../components/ui/Icon";

export type Choice<T extends string> = { value: T; label: string; description?: string; icon?: string };

export function ChoiceGroup<T extends string>({ label, value, choices, onChange, compact = false }: {
  label: string;
  value: T;
  choices: Array<Choice<T>>;
  onChange: (value: T) => void;
  compact?: boolean;
}) {
  return (
    <div className={`appearance-choice-group${compact ? " compact" : ""}`} role="radiogroup" aria-label={label}>
      {choices.map((choice) => (
        <button key={choice.value} className={`appearance-choice${value === choice.value ? " selected" : ""}`} type="button" role="radio" aria-checked={value === choice.value} onClick={() => onChange(choice.value)}>
          {choice.icon && <span className="row-icon"><Icon name={choice.icon} /></span>}
          <span><strong>{choice.label}</strong>{choice.description && <small>{choice.description}</small>}</span>
          {value === choice.value && <span className="appearance-choice-check"><Icon name="check" /></span>}
        </button>
      ))}
    </div>
  );
}
