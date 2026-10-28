import type { InputHTMLAttributes, ReactNode, TextareaHTMLAttributes } from "react";

import { cn } from "@/lib/utils";

export function Field({ label, hint, htmlFor, children }: { label: ReactNode; hint?: ReactNode; htmlFor?: string; children: ReactNode }) {
  return (
    <div className="flex min-w-0 flex-col gap-1.5">
      <label htmlFor={htmlFor} className="text-[13.5px] font-semibold text-ink">
        {label}
      </label>
      {children}
      {hint ? <span className="text-[12.5px] text-faint">{hint}</span> : null}
    </div>
  );
}

export function Input({ className, ...props }: InputHTMLAttributes<HTMLInputElement>) {
  return (
    <input
      className={cn("h-[38px] w-full min-w-0 rounded-lg border border-line-2 bg-surface px-3 text-ink outline-none focus:border-ink disabled:bg-sunken disabled:text-muted", className)}
      {...props}
    />
  );
}

export function Textarea({ className, ...props }: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return (
    <textarea
      className={cn("min-h-24 w-full resize-y rounded-lg border border-line-2 bg-surface px-3 py-2.5 leading-normal text-ink outline-none focus:border-ink", className)}
      {...props}
    />
  );
}

/** Pill tabs for a short set of choices. */
export function Segmented<T extends string>({ value, options, onChange, label }: { value: T; options: { value: T; label: string }[]; onChange: (v: T) => void; label: string }) {
  return (
    <div role="radiogroup" aria-label={label} className="inline-flex max-w-full flex-wrap gap-1 self-start rounded-[9px] bg-sunken p-[3px]">
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={value === o.value}
          onClick={() => onChange(o.value)}
          className={cn("h-[30px] cursor-pointer rounded-[7px] px-3 text-[13.5px] font-semibold text-muted", value === o.value && "bg-surface text-ink shadow-sm")}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

/** A list of options with one-line reasons; the suggested one is marked. */
export function OptionList({ options, onPick }: { options: { label: string; why?: string; recommended?: boolean }[]; onPick: (label: string) => void }) {
  return (
    <div className="flex flex-col border-t border-line">
      {options.map((o) => (
        <button
          key={o.label}
          type="button"
          onClick={() => onPick(o.label)}
          className="group grid w-full cursor-pointer grid-cols-[18px_minmax(0,1fr)] gap-3.5 border-b border-line px-1.5 py-4 text-left hover:bg-sunken"
        >
          <span className="mt-[3px] size-4 rounded-full border-[1.5px] border-line-2 group-hover:border-ink" />
          <span>
            <span className="font-semibold text-ink">{o.label}</span>
            {o.recommended ? <span className="ml-2 text-xs font-semibold text-accent">Suggested</span> : null}
            {o.why ? <span className="mt-0.5 block text-[13.5px] text-muted">{o.why}</span> : null}
          </span>
        </button>
      ))}
    </div>
  );
}
