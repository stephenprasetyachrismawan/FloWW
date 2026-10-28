import { cn } from "@/lib/utils";
import type { Prd } from "@/lib/types";

const DOT: Record<Prd["status"], string> = {
  ai: "bg-ai animate-[breathe_1.8s_ease-in-out_infinite]",
  needs: "bg-you",
  wait: "bg-faint",
  review: "bg-faint",
  done: "bg-ok",
};

export function statusText(prd: Prd): string {
  if (prd.status === "needs" && prd.action) {
    return `Your turn: ${prd.action.charAt(0).toLowerCase()}${prd.action.slice(1)}`;
  }
  return prd.sub;
}

/** Small dot + text. Color is never the only signal. */
export function StatusLine({ prd, className }: { prd: Prd; className?: string }) {
  const yours = prd.status === "needs";
  return (
    <span className={cn("inline-flex min-w-0 items-center gap-2 text-[13px] text-muted", yours && "font-semibold text-ink", className)}>
      <i className={cn("size-[7px] shrink-0 rounded-full", DOT[prd.status])} />
      <span>{statusText(prd)}</span>
    </span>
  );
}
