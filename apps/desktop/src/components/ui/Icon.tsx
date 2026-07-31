/** Renders a Material Symbols glyph without exposing it to assistive technology. */
export function Icon({ name, className }: { name: string; className?: string }) {
  return (
    <span className={`material-symbols-outlined${className ? ` ${className}` : ""}`} aria-hidden="true">
      {name}
    </span>
  );
}
