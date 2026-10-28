import { Link } from "react-router";

import { Page, PageHeader } from "@/components/page";
import { Button } from "@/components/ui/button";
import { api } from "@/lib/api";
import { useAction, useStore } from "@/lib/store";

export function DatabasePage() {
  const { wsId, workspace, prds } = useStore();
  const { run, busy } = useAction();
  if (!wsId || !workspace) return <Page>{null}</Page>;
  const { lock } = workspace;
  const db = workspace.config.workspace.db || "(not set)";
  const title = (id: string) => prds.find((p) => p.id === id)?.title ?? "";
  return (
    <Page narrow>
      <PageHeader title="Database" sub={`${db} is shared by every request in this workspace, so they take turns.`} />
      <div>
        <div className="font-serif text-[21px] leading-snug font-medium text-ink">{lock.holder ? `${lock.holder} is ${lock.doing}.` : "Free right now."}</div>
        {lock.holder ? (
          <div className="mt-1.5 text-muted">
            <Link className="underline" to={`/prd/${lock.holder}`}>
              {title(lock.holder)}
            </Link>
            . Released on its own when it finishes or stops.
          </div>
        ) : null}
      </div>
      {lock.queue.length ? (
        <div>
          <div className="mb-2.5 text-[13px] font-semibold text-muted">Waiting</div>
          <div className="border-t border-line">
            {lock.queue.map((id) => (
              <div key={id} className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-4 border-b border-line px-1 py-3.5">
                <div>
                  <div className="font-semibold text-ink">{id}</div>
                  <div className="text-[13.5px] text-muted">{title(id)}</div>
                </div>
                <Button variant="link" disabled={busy} onClick={() => run(() => api.act(wsId, id, "take-over"), `${id} goes next`)}>
                  Let it go first
                </Button>
              </div>
            ))}
          </div>
        </div>
      ) : null}
    </Page>
  );
}
