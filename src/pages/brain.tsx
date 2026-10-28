import { ExternalLink, RefreshCw } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import { Page, PageHeader } from "@/components/page";
import { Button } from "@/components/ui/button";
import { api, openExternal } from "@/lib/api";
import { useAction, useStore } from "@/lib/store";
import type { Graph, Note, Pending } from "@/lib/types";
import { cn } from "@/lib/utils";
import { ConfirmCard } from "@/pages/prd-detail";

const TABS = [
  { key: "decisions", label: "Decisions", about: "Every grill-me answer, so nobody gets asked the same thing twice." },
  { key: "rules", label: "Rules", about: "Business rules the code must keep. New ones come from approved PRDs and wait for a person to confirm." },
  { key: "people", label: "People", about: "What each person knows well and is not sure about, plus opinions. Nothing is saved without a yes from you." },
  { key: "modules", label: "Code facts", about: "Modules, models, fields and methods read straight from the code." },
  { key: "graph", label: "Graph", about: "How modules and models connect." },
] as const;

type Tab = (typeof TABS)[number]["key"];

export function BrainPage() {
  const { wsId, workspace, brainVersion } = useStore();
  const { run, busy } = useAction();
  const [tab, setTab] = useState<Tab>("decisions");
  const [notes, setNotes] = useState<Note[]>([]);
  const [opinions, setOpinions] = useState<Note[]>([]);
  const [pending, setPending] = useState<Pending[]>([]);
  const [open, setOpen] = useState<{ note: Note; text: string } | null>(null);

  useEffect(() => {
    if (!wsId) return;
    setOpen(null);
    if (tab === "graph") return;
    const folder = tab === "people" ? "understanding" : tab;
    void api.brain(wsId, folder).then(setNotes).catch(() => setNotes([]));
    if (tab === "people") {
      void api.brain(wsId, "opinions").then(setOpinions).catch(() => setOpinions([]));
      void api.pending(wsId).then(setPending).catch(() => setPending([]));
    }
  }, [wsId, tab, brainVersion]);

  if (!wsId || !workspace) return <Page>{null}</Page>;
  const about = TABS.find((t) => t.key === tab)!.about;
  const vault = `${workspace.path}/.floww/brain`;

  return (
    <Page>
      <PageHeader title="Brain" sub="Shared with the team on the floww/brain branch. Plain notes you can open in Obsidian.">
        <Button variant="link" disabled={busy} onClick={() => run(() => api.brainSync(wsId), "Brain synced")}>
          <RefreshCw className="size-3.5" />
          Sync now
        </Button>
        <Button onClick={() => openExternal(`obsidian://open?path=${encodeURIComponent(vault)}`)}>
          <ExternalLink className="size-4" />
          Open in Obsidian
        </Button>
      </PageHeader>
      <div className="flex flex-wrap gap-4.5 border-b border-line">
        {TABS.map((t) => (
          <button key={t.key} onClick={() => setTab(t.key)} className={cn("-mb-px cursor-pointer border-b-2 border-transparent pb-2.5 font-semibold text-faint hover:text-ink", tab === t.key && "border-ink text-ink")}>
            {t.label}
            {t.key === "people" && pending.length ? <span className="ml-1 text-xs text-you">{pending.length}</span> : null}
          </button>
        ))}
      </div>
      <p className="-mt-2 text-muted">{about}</p>

      {tab === "graph" ? (
        <GraphView ws={wsId} />
      ) : (
        <div className="grid grid-cols-[minmax(0,1fr)_minmax(0,1fr)] items-start gap-8 max-lg:grid-cols-1">
          <div className="flex flex-col gap-4">
            {tab === "people" ? pending.map((p) => <ConfirmCard key={p.path} ws={wsId} item={p} />) : null}
            {tab === "modules" ? (
              <div>
                <Button variant="link" disabled={busy} onClick={() => run(() => api.refreshFacts(wsId), "Code facts refreshed")}>
                  <RefreshCw className="size-3.5" />
                  Read the code again
                </Button>
              </div>
            ) : null}
            <NoteList notes={notes} onOpen={(n) => void api.brainNote(wsId, n.path).then(setOpen)} confirm={tab === "rules" ? (n) => run(() => api.confirmNote(wsId, n.path), "Rule confirmed") : undefined} />
            {tab === "people" && opinions.length ? (
              <div>
                <div className="mb-2.5 text-[13px] font-semibold text-muted">Opinions</div>
                <NoteList notes={opinions} onOpen={(n) => void api.brainNote(wsId, n.path).then(setOpen)} />
              </div>
            ) : null}
          </div>
          <div className="sticky top-4">
            {open ? (
              <div>
                <div className="mb-2 font-mono text-[12.5px] text-faint">{open.note.path}</div>
                <pre className="m-0 rounded-[10px] bg-sunken p-3.5 font-mono text-[12.5px] leading-relaxed break-words whitespace-pre-wrap">{open.text}</pre>
              </div>
            ) : (
              <p className="text-[13.5px] text-faint">Pick a note to read it.</p>
            )}
          </div>
        </div>
      )}
    </Page>
  );
}

function NoteList({ notes, onOpen, confirm }: { notes: Note[]; onOpen: (n: Note) => void; confirm?: (n: Note) => void }) {
  if (!notes.length) return <p className="text-[13.5px] text-faint">Nothing here yet. It fills up as requests go through FloWW.</p>;
  return (
    <div className="border-t border-line">
      {notes.map((n) => (
        <div key={n.path} className="grid grid-cols-[minmax(0,1fr)_auto] items-center gap-4 border-b border-line px-1 py-3">
          <button onClick={() => onOpen(n)} className="min-w-0 cursor-pointer text-left">
            <div className="font-semibold text-ink">{n.title}</div>
            {n.excerpt ? <div className="truncate text-[13.5px] text-muted">{n.excerpt}</div> : null}
            <div className="text-[12.5px] text-faint">
              {n.meta.author ? `${n.meta.author} · ` : ""}
              {n.meta.updated ?? ""}
              {n.meta.run ? ` · ${n.meta.run}` : ""}
            </div>
          </button>
          {confirm && n.trust === "draft" ? (
            <Button size="sm" onClick={() => confirm(n)}>
              Confirm
            </Button>
          ) : n.trust ? (
            <span className="inline-flex items-center gap-2 text-[13px] text-muted">
              <i className={cn("size-[7px] rounded-full", n.trust === "draft" ? "bg-you" : "bg-ok")} />
              {n.trust === "fact" ? "From code" : n.trust === "draft" ? "Draft" : "Confirmed"}
            </span>
          ) : null}
        </div>
      ))}
    </div>
  );
}

function GraphView({ ws }: { ws: string }) {
  const [g, setG] = useState<Graph | null>(null);
  const [sel, setSel] = useState<string | null>(null);
  useEffect(() => {
    void api.graph(ws).then(setG).catch(() => setG(null));
  }, [ws]);
  const layout = useMemo(() => {
    if (!g) return null;
    const modules = g.nodes.filter((n) => n.kind === "module");
    const models = g.nodes.filter((n) => n.kind === "model");
    const W = 860;
    const pos: Record<string, { x: number; y: number }> = {};
    modules.forEach((n, i) => (pos[n.id] = { x: 40 + ((W - 80) * (i + 0.5)) / Math.max(modules.length, 1), y: 60 }));
    const perRow = 6;
    models.forEach((n, i) => (pos[n.id] = { x: 40 + ((W - 80) * ((i % perRow) + 0.5)) / perRow, y: 170 + Math.floor(i / perRow) * 70 }));
    const height = 200 + Math.ceil(models.length / perRow) * 70;
    return { pos, W, height };
  }, [g]);
  if (!g || !layout) return <p className="text-faint">Loading…</p>;
  if (!g.nodes.length) return <p className="text-faint">No Odoo modules found in this repository.</p>;
  const linked = new Set(sel ? g.edges.filter((e) => e.includes(sel)).flat() : []);
  return (
    <div className="overflow-x-auto">
      <svg viewBox={`0 0 ${layout.W} ${layout.height}`} className="block h-auto w-full min-w-[640px]" role="img" aria-label="Module and model graph">
        {g.edges.map(([a, b]) => {
          const p = layout.pos[a];
          const q = layout.pos[b];
          if (!p || !q) return null;
          const hl = sel && (a === sel || b === sel);
          return <line key={a + b} x1={p.x} y1={p.y} x2={q.x} y2={q.y} stroke={hl ? "var(--muted)" : "var(--line-2)"} strokeWidth={hl ? 1.6 : 1} />;
        })}
        {g.nodes.map((n) => {
          const p = layout.pos[n.id];
          const dim = sel && !linked.has(n.id) && n.id !== sel;
          return (
            <g key={n.id} onClick={() => setSel(sel === n.id ? null : n.id)} className="cursor-pointer" opacity={dim ? 0.35 : 1}>
              <circle cx={p.x} cy={p.y} r={n.kind === "module" ? 6 : 4} fill={n.kind === "module" ? "var(--accent)" : "var(--faint)"} />
              <text x={p.x} y={p.y + 18} textAnchor="middle" fontSize="11" fill={n.id === sel ? "var(--ink)" : "var(--muted)"} fontWeight={n.id === sel ? 600 : 400}>
                {n.label}
              </text>
            </g>
          );
        })}
      </svg>
      <p className="mt-2 text-[12.5px] text-faint">Purple: modules in this repo. Grey: models. Click a dot to see its links.</p>
    </div>
  );
}
