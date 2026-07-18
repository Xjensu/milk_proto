import { Fragment, type PropsWithChildren, type RefObject, useLayoutEffect, useRef } from "react";
import { createPortal } from "react-dom";
import { X } from "lucide-react";
import { gsap } from "gsap";

export function Inspector({ title, open, onClose, triggerRef, children }: PropsWithChildren<{
  title: string;
  open: boolean;
  onClose: () => void;
  triggerRef?: RefObject<HTMLElement | null>;
}>) {
  const panelRef = useRef<HTMLElement>(null);
  const openerRef = useRef<HTMLElement | null>(null);
  const titleId = `inspector-${title.toLowerCase().replace(/\W+/g, "-")}`;

  useLayoutEffect(() => {
    if (!open || !panelRef.current) return;
    openerRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const tween = gsap.fromTo(panelRef.current, { x: reduceMotion ? 0 : 24, autoAlpha: 0 }, { x: 0, autoAlpha: 1, duration: reduceMotion ? 0 : 0.22, ease: "power2.out" });
    panelRef.current.focus();
    return () => {
      tween.kill();
      (triggerRef?.current ?? openerRef.current)?.focus();
    };
  }, [open, triggerRef]);

  if (!open) return null;
  const content = (
    <Fragment>
      <div className="inspector-backdrop" role="presentation" onMouseDown={onClose} />
      <aside
        className="admin-inspector"
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        ref={panelRef}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            onClose();
            return;
          }
          if (event.key !== "Tab" || !panelRef.current) return;

          const focusable = [...panelRef.current.querySelectorAll<HTMLElement>(
            'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [href], [tabindex]:not([tabindex="-1"])',
          )];
          if (!focusable.length) {
            event.preventDefault();
            panelRef.current.focus();
            return;
          }

          const first = focusable[0];
          const last = focusable[focusable.length - 1];
          if (event.shiftKey && document.activeElement === first) {
            event.preventDefault();
            last.focus();
          } else if (!event.shiftKey && document.activeElement === last) {
            event.preventDefault();
            first.focus();
          }
        }}
      >
        <header>
          <h2 id={titleId}>{title}</h2>
          <button type="button" aria-label="Close inspector" onClick={onClose}>
            <X aria-hidden="true" size={20} />
          </button>
        </header>
        <div className="admin-inspector-body">{children}</div>
      </aside>
    </Fragment>
  );
  // Portal to <body>: ancestors animated by gsap keep a transform, which would
  // otherwise turn position:fixed into container-relative positioning. Fall back
  // to inline rendering where no DOM exists (static markup in tests).
  return typeof document === "undefined" ? content : createPortal(content, document.body);
}
