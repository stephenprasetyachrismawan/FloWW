import { X } from "lucide-react";
import { useEffect, useRef, type ReactNode } from "react";

import { cn } from "@/lib/utils";

/** Centered dialog. Escape and the backdrop close it; focus moves inside. */
export function Modal({
  title,
  sub,
  step,
  onClose,
  children,
  footer,
  wide,
}: {
  title: string;
  sub?: ReactNode;
  step?: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  wide?: boolean;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const prev = document.activeElement as HTMLElement | null;
    const first = ref.current?.querySelector<HTMLElement>("input, textarea, button:not([aria-label='Close'])");
    first?.focus();
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("keydown", onKey);
      prev?.focus();
    };
  }, [onClose]);
  return (
    <div
      className="fixed inset-0 z-40 grid place-items-center bg-black/30 p-4"
      onMouseDown={(e) => {
        if (e.target === e.currentTarget) onClose();
      }}
    >
      <div
        ref={ref}
        role="dialog"
        aria-modal="true"
        aria-label={title}
        className={cn("flex max-h-full w-full flex-col overflow-auto rounded-[14px] border border-line bg-window shadow-pop", wide ? "max-w-[660px]" : "max-w-[560px]")}
      >
        <div className="flex items-start gap-3 px-6.5 pt-6 pb-1.5">
          <div className="flex-1">
            {step ? <div className="mb-1 text-[12.5px] text-faint">{step}</div> : null}
            <h2 className="font-serif text-[23px] leading-tight font-semibold">{title}</h2>
            {sub ? <div className="mt-1.5 text-muted">{sub}</div> : null}
          </div>
          <button aria-label="Close" onClick={onClose} className="grid size-[34px] cursor-pointer place-items-center rounded-lg text-muted hover:bg-sunken hover:text-ink">
            <X className="size-4" />
          </button>
        </div>
        <div className="flex flex-col gap-4.5 px-6.5 pt-4 pb-2">{children}</div>
        {footer ? <div className="flex flex-wrap items-center gap-4.5 px-6.5 pt-4.5 pb-6">{footer}</div> : null}
      </div>
    </div>
  );
}
