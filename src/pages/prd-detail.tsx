import { ChevronLeft, LoaderCircle } from "lucide-react";
import { useState } from "react";
import { Link, useParams } from "react-router";

import { Page } from "@/components/page";
import { PHASES, phaseOf } from "@/lib/pipeline";
import type { Prd } from "@/lib/types";
import { cn } from "@/lib/utils";

const TYPE_LABEL: Record<Prd["type"], string> = { bug: "Bug", feature: "Feature", improve: "Improvement" };
const TABS = ["Now", "PRD", "Changes", "History"] as const;

export function PrdDetailPage({ prds }: { prds: Prd[] }) {
  const { id } = useParams();
  const prd = prds.find((p) => p.id === id);
  const [tab, setTab] = useState<(typeof TABS)[number]>("Now");

  if (!prd) {
    return (
      <Page narrow>
        <BackLink />
        <p className="text-muted">No PRD with id {id}.</p>
      </Page>
    );
  }

  const current = PHASES.findIndex((p) => p.key === phaseOf(prd.step));

  return (
    <Page narrow>
      <div className="flex flex-col gap-3.5">
        <BackLink />
        <div>
          <h1 className="font-serif text-[28px] leading-tight font-semibold text-balance">{prd.title}</h1>
          <div className="mt-1.5 text-[13px] text-faint">
            {prd.id} · {TYPE_LABEL[prd.type]} · {prd.module}
          </div>
        </div>
        <ol className="flex gap-1">
          {PHASES.slice(0, 5).map((p, i) => (
            <li
              key={p.key}
              className={cn(
                "flex min-w-0 flex-1 flex-col gap-[7px] text-[12.5px] text-faint before:h-[3px] before:rounded-full before:bg-line",
                i < current && "text-muted before:bg-muted",
                i === current && "font-semibold text-ink before:bg-accent",
              )}
            >
              {p.label}
            </li>
          ))}
        </ol>
      </div>

      <div className="flex gap-[22px] border-b border-line" role="tablist">
        {TABS.map((t) => (
          <button
            key={t}
            role="tab"
            aria-selected={tab === t}
            onClick={() => setTab(t)}
            className={cn(
              "-mb-px cursor-pointer border-b-2 border-transparent pb-2.5 font-semibold text-faint hover:text-ink",
              tab === t && "border-ink text-ink",
            )}
          >
            {t}
          </button>
        ))}
      </div>

      {tab === "Now" ? (
        <div className="flex flex-col gap-5">
          <div className="font-serif text-[21px] leading-snug font-medium text-ink">{prd.status === "needs" ? prd.action : `${prd.sub}.`}</div>
          <ol className="flex flex-col gap-0.5">
            {prd.timeline.map((e, i) => (
              <li key={i} className={cn("grid grid-cols-[16px_minmax(0,1fr)] gap-2.5 py-1 text-[13.5px] text-muted", e.state === "run" && "font-semibold text-ink")}>
                <span className="grid h-[21px] place-items-center">
                  {e.state === "run" ? (
                    <LoaderCircle className="size-[13px] animate-spin" />
                  ) : (
                    <i
                      className={cn(
                        "size-[7px] rounded-full bg-line-2",
                        e.state === "fail" && "bg-bad",
                        e.state === "info" && "bg-you",
                      )}
                    />
                  )}
                </span>
                <span>{e.text}</span>
              </li>
            ))}
          </ol>
          <p className="text-[13px] text-faint">Grill, test and ship screens are wired to the pipeline engine in the next change.</p>
        </div>
      ) : (
        <p className="text-muted">{tab} arrives with the pipeline engine.</p>
      )}
    </Page>
  );
}

function BackLink() {
  return (
    <Link to="/" className="inline-flex items-center gap-1.5 text-[13.5px] font-semibold text-muted hover:text-ink">
      <ChevronLeft className="size-[15px]" />
      Board
    </Link>
  );
}
