import { Plus, Search } from "lucide-react";
import { useMemo, useState } from "react";
import { Link } from "react-router";

import { Page, PageHeader, SectionTitle } from "@/components/page";
import { StatusLine } from "@/components/status";
import { Button } from "@/components/ui/button";
import { needsYou, PHASES, phaseOf } from "@/lib/pipeline";
import { useStore } from "@/lib/store";

export function BoardPage({ onNewRequest, onAddWorkspace }: { onNewRequest: () => void; onAddWorkspace: () => void }) {
  const { workspace, prds, app } = useStore();
  const [query, setQuery] = useState("");
  const visible = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q ? prds.filter((p) => `${p.id} ${p.title} ${p.modules.join(" ")}`.toLowerCase().includes(q)) : prds;
  }, [prds, query]);

  if (!app?.workspaces.length) {
    return (
      <Page narrow>
        <PageHeader title="Add your first project" sub="FloWW works on one git repository at a time. It looks at the repo and your computer and fills in the setup for you." />
        <div>
          <Button variant="primary" onClick={onAddWorkspace}>
            <Plus className="size-4" />
            Add workspace
          </Button>
        </div>
      </Page>
    );
  }
  if (!workspace) return <Page>{null}</Page>;

  const yourTurn = prds.filter(needsYou);
  const inProgress = prds.filter((p) => p.status !== "done").length;

  return (
    <Page>
      <PageHeader title={workspace.name} sub={inProgress ? `${inProgress} in progress` : "Nothing in progress"}>
        <label className="flex h-9 w-[220px] items-center gap-2 rounded-lg border border-line-2 bg-surface px-3 text-faint focus-within:border-ink max-md:w-full">
          <Search className="size-[15px]" />
          <input className="min-w-0 flex-1 bg-transparent text-ink outline-none" placeholder="Search" aria-label="Search" value={query} onChange={(e) => setQuery(e.target.value)} />
        </label>
        <Button variant="primary" onClick={onNewRequest}>
          <Plus className="size-4" />
          New request
        </Button>
      </PageHeader>

      <section>
        <SectionTitle>Your turn</SectionTitle>
        <div className="border-t border-line">
          {yourTurn.length === 0 ? (
            <div className="border-b border-line px-1 py-3.5 text-faint">{prds.length ? "Nothing needs you right now." : "No requests yet. Log the first user requirement with New request."}</div>
          ) : (
            yourTurn.map((p) => (
              <div key={p.id} className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-4 border-b border-line px-1 py-3.5">
                <div className="min-w-0">
                  <div className="font-semibold text-ink">{p.action}</div>
                  <div className="truncate text-[13.5px] text-muted">
                    {p.title} · {p.id}
                  </div>
                </div>
                <Button size="sm" asChild>
                  <Link to={`/prd/${p.id}`}>Open</Link>
                </Button>
              </div>
            ))
          )}
        </div>
      </section>

      {prds.length ? (
        <section>
          <SectionTitle>All work</SectionTitle>
          <div className="-mx-1 overflow-x-auto px-1 pb-2">
            <div className="grid min-w-max grid-cols-[repeat(6,minmax(210px,1fr))] gap-5">
              {PHASES.map((phase) => {
                const cards = visible.filter((p) => phaseOf(p.step) === phase.key);
                return (
                  <div key={phase.key}>
                    <div className="flex items-baseline gap-2 px-0.5 pb-2.5 text-[13px] font-semibold text-muted">
                      {phase.label}
                      <span className="font-medium text-faint">{cards.length || ""}</span>
                    </div>
                    <div className="flex flex-col gap-2.5">
                      {cards.length === 0 ? <div className="h-14 rounded-[10px] border border-dashed border-line" /> : null}
                      {cards.map((p) => (
                        <Link key={p.id} to={`/prd/${p.id}`} className="flex flex-col gap-1.5 rounded-[10px] border border-line bg-surface p-3.5 transition-colors hover:border-line-2">
                          <span className="leading-snug font-semibold text-ink">{p.title}</span>
                          <span className="text-[12.5px] text-faint">
                            {p.id}
                            {p.modules.length ? ` · ${p.modules[0]}` : ""}
                          </span>
                          <StatusLine prd={p} className="mt-1 text-[12.5px]" />
                        </Link>
                      ))}
                    </div>
                  </div>
                );
              })}
            </div>
          </div>
        </section>
      ) : null}
    </Page>
  );
}
