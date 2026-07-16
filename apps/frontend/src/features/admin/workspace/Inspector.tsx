import { type PropsWithChildren, type RefObject, useLayoutEffect, useRef } from "react";
import { X } from "lucide-react";
import { gsap } from "gsap";

export function Inspector({ title, open, onClose, triggerRef, children }: PropsWithChildren<{
  title: string;
  open: boolean;
  onClose: () => void;
  triggerRef?: RefObject<HTMLElement | null>;
}>) {
  const panelRef = useRef<HTMLElement>(null);
  const titleId = `inspector-${title.toLowerCase().replace(/\W+/g, "-")}`;

  useLayoutEffect(() => {
    if (!open || !panelRef.current) return;
    const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const tween = gsap.fromTo(panelRef.current, { x: reduceMotion ? 0 : 24, autoAlpha: 0 }, { x: 0, autoAlpha: 1, duration: reduceMotion ? 0 : 0.22, ease: "power2.out" });
    panelRef.current.focus();
    return () => { tween.kill(); };
  }, [open]);

  if (!open) return null;
  return <aside className="admin-inspector" role="dialog" aria-modal="true" aria-labelledby={titleId} tabIndex={-1} ref={panelRef} onKeyDown={(event) => {
    if (event.key === "Escape") { onClose(); triggerRef?.current?.focus(); }
  }}>
    <header><h2 id={titleId}>{title}</h2><button type="button" aria-label="Close inspector" onClick={() => { onClose(); triggerRef?.current?.focus(); }}><X aria-hidden="true" size={20} /></button></header>
    <div className="admin-inspector-body">{children}</div>
  </aside>;
}
