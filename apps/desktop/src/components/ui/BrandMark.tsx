import darkLogo from "../../../../../assets/dark-logo.png";
import lightLogo from "../../../../../assets/light-logo.png";

/** Renders the product mark without replacing semantic UI icons. */
export function BrandMark({ className }: { className?: string }) {
  const suffix = className ? ` ${className}` : "";
  return (
    <span className={`brand-symbol${suffix}`} aria-hidden="true">
      <img className="brand-symbol-light" src={darkLogo} alt="" />
      <img className="brand-symbol-dark" src={lightLogo} alt="" />
    </span>
  );
}
