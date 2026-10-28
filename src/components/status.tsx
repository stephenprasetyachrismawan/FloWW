import { LoaderCircle } from "lucide-react";
import { useState } from "react";

import type { Entry, Prd } from "@/lib/types";
import { cn } from "@/lib/utils";

const DOT: Record<Prd["status"], string> = {
  ai: "bg-ai animate-[breathe_1.8s_ease-in-out_infinite]",
  needs: "bg-you",
  failed: "bg-bad",
  paused: "bg-you",
  wait: "bg-faint",
  review: "bg-faint",
  done: "bg-ok",
};

export function statusText(p: Prd): string {
  if ((p.status === "needs" || p.status === "failed" || p.status === "paused") && p.action) {
    return `Your turn: ${p.action.charAt(0).toLowerCase()}${p.action.slice(1)}`;
  }
  return p.sub;
}

/** Small dot + text. Color is never the only signal. */
export function StatusLine({ prd, className }: { prd: Prd; className?: string }) {
  const yours = prd.status === "needs" || prd.status === "failed" || prd.status === "paused";
  return (
    <span className={cn("inline-flex min-w-0 items-center gap-2 text-[13px] text-muted", yours && "font-semibold text-ink", className)}>
      <i className={cn("size-[7px] shrink-0 rounded-full", DOT[prd.status])} />
      <span className="min-w-0">{statusText(prd)}</span>
    </span>
  );
}

export function Timeline({ entries, live, limit }: { entries: Entry[]; live: boolean; limit?: number }) {
  const [cmds, setCmds] = useState(false);
  const list = limit ? entries.slice(-limit) : entries;
  const hasCmds = list.some((e) => e.cmd);
  return (
    <div className="flex flex-col gap-2">
      <ol className="flex flex-col gap-0.5">
        {list.map((e, i) => (
          <li key={`${e.at}-${i}`} className={cn("grid grid-cols-[16px_minmax(0,1fr)] gap-2.5 py-1 text-[13.5px] text-muted", e.state === "run" && live && "font-semibold text-ink")}>
            <span className="grid h-[21px] place-items-center">
              {e.state === "run" && live ? (
                <LoaderCircle className="size-[13px] animate-spin" />
              ) : (
                <i className={cn("size-[7px] rounded-full bg-line-2", e.state === "fail" && "bg-bad", e.state === "info" && "bg-you")} />
              )}
            </span>
            <span className="min-w-0">
              {e.text}
              {cmds && e.cmd ? <span className="mt-1 block font-mono text-xs font-normal break-words whitespace-pre-wrap text-faint">{e.cmd}</span> : null}
            </span>
          </li>
        ))}
      </ol>
      {hasCmds ? (
        <button onClick={() => setCmds(!cmds)} className="cursor-pointer self-start text-[13px] font-semibold text-muted hover:text-ink">
          {cmds ? "Hide commands" : "Show commands"}
        </button>
      ) : null}
    </div>
  );
}
