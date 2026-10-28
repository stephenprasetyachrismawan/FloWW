import type { ReactNode } from "react";

import { cn } from "@/lib/utils";

export function Page({ children, narrow }: { children: ReactNode; narrow?: boolean }) {
  return (
    <div className={cn("flex min-w-0 flex-col gap-7 px-9 pt-8 pb-12", narrow && "mx-auto w-full max-w-[800px]")}>
      {children}
    </div>
  );
}

export function PageHeader({ title, sub, children }: { title: string; sub?: ReactNode; children?: ReactNode }) {
  return (
    <div className="flex flex-wrap items-end gap-4">
      <div className="min-w-[200px] flex-1">
        <h1 className="font-serif text-[28px] leading-tight font-semibold tracking-[-0.01em] text-balance">{title}</h1>
        {sub ? <div className="mt-1 text-muted">{sub}</div> : null}
      </div>
      {children}
    </div>
  );
}

export function SectionTitle({ children }: { children: ReactNode }) {
  return <div className="mb-2.5 text-[13px] font-semibold text-muted">{children}</div>;
}
