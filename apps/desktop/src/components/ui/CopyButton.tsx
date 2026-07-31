import { useState } from "react";
import { Icon } from "./Icon";

/** Copies a value to the clipboard and briefly displays confirmation. */
export function CopyButton({ value, label }: { value: string; label: string }) {
  const [copied, setCopied] = useState(false);

  const copy = async () => {
    await navigator.clipboard.writeText(value);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1600);
  };

  return <button type="button" aria-label={label} onClick={copy}><Icon name={copied ? "check" : "content_copy"} className={copied ? "icon-confirm" : ""} /></button>;
}
