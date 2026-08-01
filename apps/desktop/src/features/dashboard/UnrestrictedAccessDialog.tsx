import { createPortal } from "react-dom";
import { useEffect, useRef } from "react";
import { Icon } from "../../components/ui/Icon";

/** Confirms switching filesystem access from roots to unrestricted mode. */
export function UnrestrictedAccessDialog({ onCancel, onConfirm }: {
  onCancel: () => void;
  onConfirm: () => void;
}) {
  const cancelRef = useRef<HTMLButtonElement>(null);
  const dialogRef = useRef<HTMLElement>(null);

  useEffect(() => cancelRef.current?.focus(), []);

  useEffect(() => {
    const handleKeys = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onCancel();
        return;
      }
      if (event.key === "Enter") {
        event.preventDefault();
        onConfirm();
        return;
      }
      if (event.key !== "Tab") return;
      const focusable = Array.from(dialogRef.current?.querySelectorAll<HTMLElement>("button") ?? []);
      const first = focusable[0];
      const last = focusable[focusable.length - 1];
      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last?.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first?.focus();
      }
    };
    window.addEventListener("keydown", handleKeys);
    return () => window.removeEventListener("keydown", handleKeys);
  }, [onCancel, onConfirm]);

  return createPortal(
    <div className="dialog-scrim" onMouseDown={(event) => event.target === event.currentTarget && onCancel()}>
      <section ref={dialogRef} className="profile-dialog" role="dialog" aria-modal="true" aria-labelledby="unrestricted-dialog-title" aria-describedby="unrestricted-dialog-description">
        <span className="profile-dialog-icon"><Icon name="folder_open" /></span>
        <div className="profile-dialog-copy">
          <h2 id="unrestricted-dialog-title">Enable unrestricted access?</h2>
          <p id="unrestricted-dialog-description">Filesystem tools will be able to access any path available to your user account. Write operations remain controlled by the selected permissions.</p>
        </div>
        <div className="profile-dialog-actions">
          <button ref={cancelRef} className="text-button" type="button" onClick={onCancel}>Cancel</button>
          <button className="primary-button" type="button" onClick={onConfirm}>Enable</button>
        </div>
      </section>
    </div>,
    document.querySelector(".desktop-stage") ?? document.body,
  );
}
