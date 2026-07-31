import { createPortal } from "react-dom";
import { useEffect, useRef, useState, type FormEvent } from "react";
import type { ProfileDialogKind } from "../../types";
import { Icon } from "../../components/ui/Icon";

const content = {
  create: { icon: "create_new_folder", title: "Create profile", description: "Start with the default server configuration.", action: "Create" },
  duplicate: { icon: "content_copy", title: "Duplicate profile", description: "Copy the current profile and all of its settings.", action: "Duplicate" },
  rename: { icon: "drive_file_rename_outline", title: "Rename profile", description: "Choose a name that makes this configuration easy to identify.", action: "Rename" },
} satisfies Record<ProfileDialogKind, { icon: string; title: string; description: string; action: string }>;

/** Modal used by create, duplicate, and rename profile operations. */
export function ProfileNameDialog({ kind, initialName, onCancel, onSubmit }: {
  kind: ProfileDialogKind;
  initialName: string;
  onCancel: () => void;
  onSubmit: (name: string) => Promise<string | null>;
}) {
  const [name, setName] = useState(initialName);
  const [error, setError] = useState("");
  const [submitting, setSubmitting] = useState(false);
  const inputRef = useRef<HTMLInputElement>(null);
  const dialogRef = useRef<HTMLElement>(null);
  const dialogContent = content[kind];

  useEffect(() => {
    inputRef.current?.focus();
    inputRef.current?.select();
  }, []);

  useEffect(() => {
    const handleKeys = (event: KeyboardEvent) => {
      if (event.key === "Escape" && !submitting) return onCancel();
      if (event.key !== "Tab") return;
      const focusable = Array.from(dialogRef.current?.querySelectorAll<HTMLElement>("button:not(:disabled), input:not(:disabled)") ?? []);
      if (!focusable.length) return;
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    };
    window.addEventListener("keydown", handleKeys);
    return () => window.removeEventListener("keydown", handleKeys);
  }, [onCancel, submitting]);

  const submit = async (event: FormEvent) => {
    event.preventDefault();
    const trimmedName = name.trim();
    if (!trimmedName) {
      setError("Enter a profile name");
      inputRef.current?.focus();
      return;
    }
    setSubmitting(true);
    setError("");
    const actionError = await onSubmit(trimmedName);
    if (actionError) {
      setError(actionError);
      setSubmitting(false);
    } else onCancel();
  };

  return createPortal(
    <div className="dialog-scrim" onMouseDown={(event) => event.target === event.currentTarget && !submitting && onCancel()}>
      <section ref={dialogRef} className="profile-dialog" role="dialog" aria-modal="true" aria-labelledby="profile-dialog-title" aria-describedby="profile-dialog-description">
        <span className="profile-dialog-icon"><Icon name={dialogContent.icon} /></span>
        <div className="profile-dialog-copy"><h2 id="profile-dialog-title">{dialogContent.title}</h2><p id="profile-dialog-description">{dialogContent.description}</p></div>
        <form onSubmit={submit}>
          <label className={`dialog-field ${error ? "invalid" : ""}`}><span>Profile name</span><input ref={inputRef} value={name} onChange={(event) => { setName(event.target.value); setError(""); }} disabled={submitting} aria-invalid={Boolean(error)} aria-describedby={error ? "profile-dialog-error" : undefined} spellCheck={false} /></label>
          {error && <p className="dialog-field-error" id="profile-dialog-error" role="alert"><Icon name="error" />{error}</p>}
          <div className="profile-dialog-actions"><button className="text-button" type="button" onClick={onCancel} disabled={submitting}>Cancel</button><button className="primary-button" type="submit" disabled={submitting || !name.trim()}>{submitting && <Icon name="progress_activity" />}{submitting ? `${dialogContent.action}...` : dialogContent.action}</button></div>
        </form>
      </section>
    </div>,
    document.querySelector(".desktop-stage") ?? document.body,
  );
}
