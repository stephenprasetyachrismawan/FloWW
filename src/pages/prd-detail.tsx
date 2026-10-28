import { ChevronDown, ChevronLeft, ChevronRight, Copy, ExternalLink, LoaderCircle, Play } from "lucide-react";
import { useEffect, useState } from "react";
import { Link, useParams } from "react-router";

import { Page } from "@/components/page";
import { Timeline } from "@/components/status";
import { Button } from "@/components/ui/button";
import { Field, Input, OptionList, Textarea } from "@/components/ui/field";
import { api, openExternal } from "@/lib/api";
import { PHASES, phaseOf, TYPE_LABEL } from "@/lib/pipeline";
import { useAction, useStore } from "@/lib/store";
import type { Pending, Prd } from "@/lib/types";
import { cn } from "@/lib/utils";

const TABS = ["Now", "PRD", "Changes", "History"] as const;

export function PrdDetailPage() {
  const { id } = useParams();
  const { prds, wsId } = useStore();
  const prd = prds.find((p) => p.id === id);
  const [tab, setTab] = useState<(typeof TABS)[number]>("Now");

  if (!prd || !wsId) {
    return (
      <Page narrow>
        <BackLink />
        <p className="text-muted">{prds.length ? `No request with id ${id} in this workspace.` : "Loading…"}</p>
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
            {prd.id} · {TYPE_LABEL[prd.type]}
            {prd.modules.length ? ` · ${prd.modules.join(", ")}` : ""}
            {prd.reporter ? ` · asked by ${prd.reporter}` : ""}
          </div>
        </div>
        <ol className="flex gap-1">
          {PHASES.slice(0, 5).map((p, i) => (
            <li
              key={p.key}
              className={cn(
                "flex min-w-0 flex-1 flex-col gap-[7px] text-[12.5px] text-faint before:h-[3px] before:rounded-full before:bg-line",
                (i < current || current === 5) && "text-muted before:bg-muted",
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
            className={cn("-mb-px cursor-pointer border-b-2 border-transparent pb-2.5 font-semibold text-faint hover:text-ink", tab === t && "border-ink text-ink")}
          >
            {t}
          </button>
        ))}
      </div>
      {tab === "Now" ? <Now prd={prd} ws={wsId} /> : null}
      {tab === "PRD" ? <PrdTab prd={prd} ws={wsId} /> : null}
      {tab === "Changes" ? <ChangesTab prd={prd} ws={wsId} /> : null}
      {tab === "History" ? <HistoryTab prd={prd} /> : null}
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

function Lead({ title, sub }: { title: string; sub?: string }) {
  return (
    <div>
      <div className="font-serif text-[21px] leading-snug font-medium text-balance text-ink">{title}</div>
      {sub ? <div className="mt-1.5 text-muted">{sub}</div> : null}
    </div>
  );
}

function Note({ children }: { children: React.ReactNode }) {
  return <div className="rounded-[10px] bg-sunken px-3.5 py-3 text-[13.5px]">{children}</div>;
}

function Now({ prd, ws }: { prd: Prd; ws: string }) {
  const g = prd.grill;
  if (prd.step === "grill_prd" && prd.status === "needs" && g) return <GrillView prd={prd} ws={ws} />;
  if (prd.step === "approve_prd" && prd.status === "needs") return <ApproveView prd={prd} ws={ws} />;
  if (["start_odoo", "upgrade_modules", "human_test"].includes(prd.step) && prd.status !== "failed" && prd.status !== "paused") return <TestView prd={prd} ws={ws} />;
  if (["commit_message", "commit", "push", "pull_request"].includes(prd.step) && prd.ship && prd.status !== "failed") return <ShipView prd={prd} ws={ws} />;
  if (prd.status === "done") return <DoneView prd={prd} />;
  return <WorkView prd={prd} ws={ws} />;
}

function WorkView({ prd, ws }: { prd: Prd; ws: string }) {
  const { workspace } = useStore();
  const { run, busy } = useAction();
  const [hint, setHint] = useState<string | null>(null);
  const live = prd.status === "ai" || prd.status === "wait";
  const stuck = prd.status === "failed" || prd.status === "paused" || (prd.status === "needs" && prd.step === "develop");
  const holder = workspace?.lock.holder;
  const similar = prd.context?.similar;
  return (
    <div className="flex flex-col gap-5.5">
      {similar ? (
        <Note>
          This looks like <Link className="font-semibold text-ink underline" to={`/prd/${similar}`}>{similar}</Link>. Check it before going on.
        </Note>
      ) : null}
      <Lead
        title={prd.status === "paused" ? "Paused." : prd.status === "failed" ? "Stopped." : `${prd.sub}.`}
        sub={
          prd.status === "failed"
            ? prd.sub
            : prd.status === "wait" && holder
              ? `${holder} is ${workspace?.lock.doing || "using the database"}. This continues on its own when it is free.`
              : prd.status === "needs" && prd.step === "develop"
                ? "The AI could not make the tests pass. Give it a hint, then continue."
                : undefined
        }
      />
      <Timeline entries={prd.timeline} live={live} limit={8} />
      <div className="flex flex-wrap items-center gap-4.5">
        {stuck ? (
          <Button variant="primary" disabled={busy} onClick={() => run(() => api.act(ws, prd.id, "resume"))}>
            {prd.action === "Try again" ? "Try again" : "Continue"}
          </Button>
        ) : null}
        {prd.status === "ai" ? (
          <Button variant="link" disabled={busy} onClick={() => run(() => api.act(ws, prd.id, "pause"))}>
            Pause
          </Button>
        ) : null}
        {prd.status === "wait" ? (
          <Button variant="link" disabled={busy} onClick={() => run(() => api.act(ws, prd.id, "take-over"), "Database handed over")}>
            I need the database now
          </Button>
        ) : null}
        {["develop", "feedback"].includes(prd.step) || stuck ? (
          <Button variant="link" onClick={() => setHint(hint === null ? "" : null)}>
            Give a hint
          </Button>
        ) : null}
      </div>
      {hint !== null ? (
        <Field label="Your hint" htmlFor="hint" hint="Used in the next AI session for this request.">
          <Textarea id="hint" value={hint} onChange={(e) => setHint(e.target.value)} placeholder="For example: backorders are created through procurement_group_id" />
          <div className="flex gap-4">
            <Button
              size="sm"
              variant="primary"
              disabled={!hint.trim() || busy}
              onClick={async () => {
                if (await run(() => api.act(ws, prd.id, "hint", { text: hint }), "Hint saved")) setHint(null);
              }}
            >
              Save hint
            </Button>
          </div>
        </Field>
      ) : null}
    </div>
  );
}

function usePending(ws: string, prdId: string) {
  const { brainVersion } = useStore();
  const [items, setItems] = useState<Pending[]>([]);
  useEffect(() => {
    void api.pending(ws).then((all) => setItems(all.filter((p) => p.source === prdId))).catch(() => setItems([]));
  }, [ws, prdId, brainVersion]);
  return items;
}

export function ConfirmCard({ ws, item }: { ws: string; item: Pending }) {
  const { run, busy } = useAction();
  const [edit, setEdit] = useState<string | null>(null);
  return (
    <div className="flex flex-col gap-3 rounded-[10px] border border-line bg-surface p-4">
      <p className="text-ink">Should I remember this? “{item.text}”</p>
      <small className="text-faint">From {item.source}. Saved to the shared brain so the AI can explain things better next time.</small>
      {edit !== null ? (
        <div className="flex gap-2.5">
          <Input value={edit} onChange={(e) => setEdit(e.target.value)} aria-label="Edit note" />
          <Button size="sm" variant="primary" disabled={busy} onClick={() => run(() => api.resolvePending(ws, item.path, true, edit))}>
            Save
          </Button>
        </div>
      ) : (
        <div className="flex flex-wrap items-center gap-4">
          <Button size="sm" disabled={busy} onClick={() => run(() => api.resolvePending(ws, item.path, true), "Remembered")}>
            Yes, remember
          </Button>
          <Button variant="link" onClick={() => setEdit(item.text)}>
            Edit
          </Button>
          <Button variant="link" disabled={busy} onClick={() => run(() => api.resolvePending(ws, item.path, false))}>
            No
          </Button>
        </div>
      )}
    </div>
  );
}

function GrillView({ prd, ws }: { prd: Prd; ws: string }) {
  const { run, busy } = useAction();
  const g = prd.grill!;
  const [other, setOther] = useState<string | null>(null);
  const [showAnswers, setShowAnswers] = useState(false);
  const pending = usePending(ws, prd.id);
  const answered = g.answers.length;
  const answers = answered ? (
    <div>
      <button onClick={() => setShowAnswers(!showAnswers)} className="inline-flex cursor-pointer items-center gap-1.5 text-[13.5px] font-semibold text-muted hover:text-ink">
        {showAnswers ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />}
        {answered} answered
      </button>
      {showAnswers ? (
        <div className="mt-3 flex flex-col gap-3 border-l-2 border-line pl-3.5">
          {g.answers.map((a, i) => (
            <div key={i}>
              <div className="text-[13px] text-muted">{a.question.text}</div>
              <div className="font-semibold text-ink">{a.answer}</div>
            </div>
          ))}
        </div>
      ) : null}
    </div>
  ) : null;

  if (!g.current) {
    return (
      <div className="flex flex-col gap-5.5">
        <Lead title="That is everything the AI needed to ask." sub="Your answers are saved to the brain. Write the PRD now." />
        {pending.map((p) => (
          <ConfirmCard key={p.path} ws={ws} item={p} />
        ))}
        {answers}
        <div>
          <Button variant="primary" disabled={busy} onClick={() => run(() => api.act(ws, prd.id, "finish-grill"))}>
            Write the PRD
          </Button>
        </div>
      </div>
    );
  }
  const q = g.current;
  const pick = (answer: string, own: boolean) => run(() => api.act(ws, prd.id, "answer", { answer, own })).then(() => setOther(null));
  return (
    <div className="flex flex-col gap-5.5">
      <div>
        <div className="text-[13px] text-faint">
          Question {answered + 1} · {q.topic}
        </div>
        <div className="mt-1.5 font-serif text-2xl leading-snug font-medium text-balance text-ink">{q.text}</div>
      </div>
      {q.explain ? (
        <div className="border-l-2 border-accent py-0.5 pl-3.5 text-sm">
          <b className="text-ink">Quick context.</b> {q.explain}
        </div>
      ) : null}
      <div className={cn(busy && "pointer-events-none opacity-50")}>
        <OptionList options={q.options.map((o) => ({ label: o.label, why: o.why, recommended: o.recommended }))} onPick={(l) => pick(l, false)} />
      </div>
      {other !== null ? (
        <div className="flex gap-2.5">
          <Input autoFocus value={other} onChange={(e) => setOther(e.target.value)} placeholder="Type your answer" aria-label="Your own answer" onKeyDown={(e) => e.key === "Enter" && other.trim() && pick(other, true)} />
          <Button disabled={!other.trim() || busy} onClick={() => pick(other, true)}>
            Use this
          </Button>
        </div>
      ) : (
        <div className="flex flex-wrap items-center gap-4.5">
          <Button variant="link" onClick={() => setOther("")}>
            Something else…
          </Button>
          <span className="flex-1" />
          {answered ? (
            <Button variant="link" disabled={busy} onClick={() => run(() => api.act(ws, prd.id, "finish-grill"))}>
              Stop here and write the PRD
            </Button>
          ) : null}
        </div>
      )}
      {busy ? (
        <p className="flex items-center gap-2 text-[13.5px] text-muted">
          <LoaderCircle className="size-4 animate-spin" />
          Saving your answer…
        </p>
      ) : null}
      {pending.map((p) => (
        <ConfirmCard key={p.path} ws={ws} item={p} />
      ))}
      {answers}
    </div>
  );
}

function usePrdHtml(ws: string, prd: Prd) {
  const [html, setHtml] = useState<{ html: string; markdown: string } | null>(null);
  useEffect(() => {
    if (!prd.prd_markdown) return;
    void api.prdHtml(ws, prd.id).then(setHtml).catch(() => setHtml(null));
  }, [ws, prd.id, prd.prd_markdown]);
  return html;
}

function PrdDoc({ html }: { html: string }) {
  return (
    <article
      className="doc max-w-[66ch] text-text [&_h1]:font-serif [&_h1]:text-[26px] [&_h1]:font-semibold [&_h2]:mt-7 [&_h2]:mb-2 [&_h2]:text-[15px] [&_h2]:font-semibold [&_h3]:mt-5 [&_h3]:font-semibold [&_li]:my-0.5 [&_p]:my-2 [&_pre]:rounded-lg [&_pre]:bg-sunken [&_pre]:p-3 [&_pre]:font-mono [&_pre]:text-[12.5px] [&_pre]:whitespace-pre-wrap [&_table]:my-2 [&_table]:w-full [&_table]:text-[13.5px] [&_td]:border-b [&_td]:border-line [&_td]:py-2 [&_td]:pr-2 [&_th]:border-b [&_th]:border-line [&_th]:py-2 [&_th]:pr-2 [&_th]:text-left [&_th]:text-muted [&_ul]:list-disc [&_ul]:pl-5 [&_ol]:list-decimal [&_ol]:pl-5"
      // HTML comes from our own markdown renderer, which turns raw HTML into text.
      dangerouslySetInnerHTML={{ __html: html }}
    />
  );
}

function ApproveView({ prd, ws }: { prd: Prd; ws: string }) {
  const { run, busy } = useAction();
  const html = usePrdHtml(ws, prd);
  return (
    <div className="flex flex-col gap-5.5">
      <Lead title="The PRD is ready." sub="Read it through. The AI starts building only after you approve." />
      {html ? <PrdDoc html={html.html} /> : <p className="text-muted">Loading the PRD…</p>}
      <div>
        <div className="mb-2 text-[13px] font-semibold text-muted">Acceptance criteria the AI will build, one session each</div>
        <ol className="list-decimal pl-5 text-[13.5px]">
          {prd.criteria.map((c, i) => (
            <li key={i} className="py-1">
              Given {c.given}, when {c.when}, then {c.then}.
            </li>
          ))}
        </ol>
      </div>
      <div className="sticky bottom-0 flex flex-wrap items-center gap-4.5 bg-gradient-to-b from-transparent to-window to-30% pt-7 pb-1">
        <Button variant="primary" disabled={busy} onClick={() => run(() => api.act(ws, prd.id, "approve"))}>
          Approve and start building
        </Button>
        <Button variant="link" disabled={busy} onClick={() => run(() => api.act(ws, prd.id, "reopen-grill"))}>
          Ask me more questions
        </Button>
      </div>
    </div>
  );
}

function TestView({ prd, ws }: { prd: Prd; ws: string }) {
  const { workspace } = useStore();
  const { run, busy } = useAction();
  const [open, setOpen] = useState<number | null>(null);
  const port = workspace?.config.workspace.http_port ?? 8069;
  const db = workspace?.config.workspace.db ?? "";
  const live = prd.odoo === "on";
  const noOdoo = !workspace?.config.workspace.launch_config;
  const canMark = live || noOdoo;
  const done = prd.tests.filter((t) => t.result).length;
  const allPass = prd.tests.length > 0 && prd.tests.every((t) => t.result === "pass");
  const anyFeedback = prd.tests.some((t) => t.result === "fail" || t.note.trim());
  const holder = workspace?.lock.holder;
  return (
    <div className="flex flex-col gap-5.5">
      <Lead
        title={live ? "Try each case in Odoo." : "Test it yourself in Odoo."}
        sub={`${prd.feedback_rounds ? "The AI worked on what you found. Check again. " : ""}${prd.tests.length} cases from the PRD. Mark each one.`}
      />
      <div className="flex flex-wrap items-center gap-3.5 rounded-[10px] border border-line bg-surface px-4 py-3.5">
        {prd.status === "ai" || prd.odoo === "starting" ? <LoaderCircle className="size-4 animate-spin" /> : <i className={cn("size-[7px] rounded-full", live ? "bg-ok" : "bg-faint")} />}
        <div className="min-w-[200px] flex-1">
          <b className="text-ink">
            {live ? "Odoo is running" : prd.status === "wait" ? "Waiting for the database" : prd.status === "ai" ? prd.sub : noOdoo ? "No Odoo launch config set" : "Odoo is not running"}
          </b>
          <div className="text-[13px] text-muted">
            {live
              ? `localhost:${port}${db ? ` · ${db}` : ""}`
              : prd.status === "wait" && holder
                ? `${holder} is ${workspace?.lock.doing}.`
                : noOdoo
                  ? "Set one in Settings to start Odoo from here, or test your usual way."
                  : `Starts this branch on ${db} and upgrades ${prd.modules.join(", ") || "the changed modules"}.`}
          </div>
        </div>
        {live ? (
          <>
            <Button variant="primary" onClick={() => openExternal(`http://localhost:${port}/odoo`)}>
              <ExternalLink className="size-4" />
              Open Odoo
            </Button>
            <Button variant="link" disabled={busy} onClick={() => run(() => api.act(ws, prd.id, "stop-odoo"))}>
              Stop
            </Button>
          </>
        ) : prd.status === "wait" ? (
          <Button variant="link" disabled={busy} onClick={() => run(() => api.act(ws, prd.id, "take-over"))}>
            I need it now
          </Button>
        ) : !noOdoo && prd.status === "needs" ? (
          <Button variant="primary" disabled={busy} onClick={() => run(() => api.act(ws, prd.id, "start-odoo"))}>
            <Play className="size-3.5" />
            Start Odoo
          </Button>
        ) : null}
      </div>
      <div className="border-t border-line">
        {prd.tests.map((t, i) => {
          const expanded = open === i || t.result === "fail";
          return (
            <div key={i} className="border-b border-line px-1 py-3.5">
              <div className="flex flex-wrap items-center gap-3">
                <button onClick={() => setOpen(open === i ? null : i)} className="flex min-w-[200px] flex-1 cursor-pointer items-center gap-2.5 text-left font-semibold text-ink">
                  <span className={cn("grid size-4 shrink-0 place-items-center rounded-full border-[1.5px] border-line-2 text-[10px] text-white", t.result === "pass" && "border-ok bg-ok", t.result === "fail" && "border-bad bg-bad")}>
                    {t.result === "pass" ? "✓" : t.result === "fail" ? "✕" : ""}
                  </span>
                  <span>{t.title}</span>
                  {t.kind.toLowerCase() === "edge" ? <span className="text-xs font-medium text-faint">edge case</span> : null}
                </button>
                <div className="flex gap-1.5">
                  {(["pass", "fail"] as const).map((r) => (
                    <button
                      key={r}
                      disabled={!canMark || busy}
                      onClick={() => run(() => api.act(ws, prd.id, "test", t.result === r ? { index: i, clear: true } : { index: i, result: r }))}
                      className={cn(
                        "h-7 cursor-pointer rounded-[7px] border border-line px-2.5 text-[13px] font-semibold text-muted hover:border-line-2 hover:text-ink disabled:cursor-not-allowed disabled:opacity-40",
                        t.result === r && (r === "pass" ? "border-ok text-ok" : "border-bad text-bad"),
                      )}
                    >
                      {r === "pass" ? "Works" : "Broken"}
                    </button>
                  ))}
                </div>
              </div>
              {expanded ? (
                <dl className="mt-2.5 ml-6.5 grid grid-cols-[60px_minmax(0,1fr)] gap-x-3 gap-y-1 text-[13.5px]">
                  <dt className="text-faint">Do</dt>
                  <dd>{t.steps}</dd>
                  {t.data ? (
                    <>
                      <dt className="text-faint">Data</dt>
                      <dd className="font-mono text-[13px]">{t.data}</dd>
                    </>
                  ) : null}
                  <dt className="text-faint">Expect</dt>
                  <dd>{t.expected}</dd>
                  <NoteBox ws={ws} prd={prd} index={i} value={t.note} disabled={!canMark} />
                </dl>
              ) : null}
            </div>
          );
        })}
      </div>
      <div className="flex flex-wrap items-center gap-4.5">
        {allPass ? (
          <Button variant="primary" disabled={busy} onClick={() => run(() => api.act(ws, prd.id, "all-good"))}>
            Everything works, ship it
          </Button>
        ) : (
          <Button variant="primary" disabled={!anyFeedback || busy} onClick={() => run(() => api.act(ws, prd.id, "feedback"))}>
            Send to AI to fix
          </Button>
        )}
        <span className="text-[13px] text-faint">
          {done} of {prd.tests.length} checked
        </span>
      </div>
    </div>
  );
}

function NoteBox({ ws, prd, index, value, disabled }: { ws: string; prd: Prd; index: number; value: string; disabled: boolean }) {
  const [text, setText] = useState(value);
  useEffect(() => setText(value), [value]);
  return (
    <Textarea
      className="col-span-2 mt-2 min-h-[60px]"
      rows={2}
      disabled={disabled}
      placeholder="What did you see? (optional)"
      value={text}
      onChange={(e) => setText(e.target.value)}
      onBlur={() => {
        if (text !== value) void api.act(ws, prd.id, "test", { index, note: text });
      }}
    />
  );
}

function ShipView({ prd, ws }: { prd: Prd; ws: string }) {
  const { run, busy } = useAction();
  const s = prd.ship!;
  const [msg, setMsg] = useState(s.message);
  const [title, setTitle] = useState(s.pr_title);
  const [body, setBody] = useState(s.pr_body);
  useEffect(() => setMsg(s.message), [s.message]);
  const steps = ["Commit", "Push", "Pull request"];
  const leads = ["Check the commit message.", "Committed. Push the branch?", "Pushed. Open a pull request?", "Pull request is open."];
  return (
    <div className="flex flex-col gap-5.5">
      <div>
        <div className="font-serif text-[21px] leading-snug font-medium text-ink">{leads[Math.min(s.stage, 3)]}</div>
        <div className="mt-1.5 text-muted">
          {steps.map((x, i) => (
            <span key={x}>
              {i ? " → " : ""}
              {i < s.stage ? <s>{x}</s> : i === s.stage ? <b className="text-ink">{x}</b> : x}
            </span>
          ))}
        </div>
      </div>
      {s.stage === 0 ? (
        <>
          <Field label="Commit message" htmlFor="cm" hint="Drafted by the AI. All work on this branch becomes this one commit.">
            <Textarea id="cm" className="min-h-[150px] font-mono text-[13px]" value={msg} onChange={(e) => setMsg(e.target.value)} />
          </Field>
          <div>
            <Button variant="primary" disabled={busy || !msg.trim()} onClick={() => run(() => api.act(ws, prd.id, "commit", { message: msg }))}>
              Commit
            </Button>
          </div>
        </>
      ) : null}
      {s.stage === 1 ? (
        <>
          <Note>
            <span className="font-mono">{s.commit?.slice(0, 7)}</span> {s.message.split("\n")[0]}
          </Note>
          <div>
            <Button variant="primary" disabled={busy} onClick={() => run(() => api.act(ws, prd.id, "push"))}>
              Push branch
            </Button>
          </div>
        </>
      ) : null}
      {s.stage === 2 ? (
        <>
          <Field label="Title" htmlFor="pt">
            <Input id="pt" value={title} onChange={(e) => setTitle(e.target.value)} />
          </Field>
          <Field label="Description" htmlFor="pb">
            <Textarea id="pb" value={body} onChange={(e) => setBody(e.target.value)} />
          </Field>
          <div className="flex flex-wrap items-center gap-4.5">
            <Button variant="primary" disabled={busy || !title.trim()} onClick={() => run(() => api.act(ws, prd.id, "pr", { title, body }))}>
              Open pull request
            </Button>
            <Button variant="link" disabled={busy} onClick={() => run(() => api.act(ws, prd.id, "skip-pr"))}>
              Skip
            </Button>
          </div>
        </>
      ) : null}
      {s.stage >= 3 && s.pr_url ? (
        <div className="flex flex-wrap items-center gap-4.5">
          <Button onClick={() => openExternal(s.pr_url!)}>
            <ExternalLink className="size-4" />
            Pull request #{s.pr_number}
          </Button>
          <Button
            variant="link"
            disabled={busy}
            onClick={async () => {
              const r = await run(() => api.act(ws, prd.id, "check-pr"));
              if (r && r._state !== "merged") await run(async () => Promise.reject(new Error(`Still ${r._state ?? "open"}.`)));
            }}
          >
            Check if it is merged
          </Button>
        </div>
      ) : null}
    </div>
  );
}

function DoneView({ prd }: { prd: Prd }) {
  return (
    <div className="flex flex-col gap-5.5">
      <Lead title={prd.sub === "Merged" ? "Merged." : "Done."} sub="The brain has the notes and the worktree is cleaned up." />
      <Timeline entries={prd.timeline} live={false} limit={6} />
    </div>
  );
}

function PrdTab({ prd, ws }: { prd: Prd; ws: string }) {
  const { run, busy } = useAction();
  const { toast } = useStore();
  const html = usePrdHtml(ws, prd);
  const [edit, setEdit] = useState<string | null>(null);
  if (!prd.prd_markdown) return <p className="text-muted">The PRD is written after the questions are answered.</p>;
  const copy = async (text: string, what: string) => {
    try {
      await navigator.clipboard.writeText(text);
      toast(`${what} copied`, "", "ok");
    } catch {
      toast("Copy did not work here", "Select the text and copy it yourself", "bad");
    }
  };
  return (
    <div className="flex flex-col gap-5.5">
      <div className="flex flex-wrap gap-4.5">
        <Button variant="link" onClick={() => html && copy(html.html, "HTML")}>
          <Copy className="size-3.5" />
          Copy HTML
        </Button>
        <Button variant="link" onClick={() => copy(prd.prd_markdown ?? "", "Markdown")}>
          Copy Markdown
        </Button>
        <Button variant="link" onClick={() => setEdit(edit === null ? (prd.prd_markdown ?? "") : null)}>
          {edit === null ? "Edit" : "Close editor"}
        </Button>
      </div>
      {edit !== null ? (
        <Field label={`brain/prd/${prd.id}.md`} htmlFor="md">
          <Textarea id="md" className="min-h-[260px] font-mono text-[13px]" value={edit} onChange={(e) => setEdit(e.target.value)} />
          <div>
            <Button size="sm" variant="primary" disabled={busy} onClick={async () => (await run(() => api.act(ws, prd.id, "edit-prd", { markdown: edit }), "PRD saved")) && setEdit(null)}>
              Save
            </Button>
          </div>
        </Field>
      ) : null}
      {html ? <PrdDoc html={html.html} /> : null}
    </div>
  );
}

function ChangesTab({ prd, ws }: { prd: Prd; ws: string }) {
  const [d, setD] = useState<{ files: string[]; stat?: string; diff: string } | null>(null);
  useEffect(() => {
    void api.diff(ws, prd.id).then(setD).catch(() => setD(null));
  }, [ws, prd.id, prd.timeline.length]);
  if (!prd.branch && !d?.files.length) return <p className="text-muted">No code changes yet.</p>;
  return (
    <div className="flex flex-col gap-5.5">
      <div>
        <div className="mb-2.5 text-[13px] font-semibold text-muted">Files on {prd.branch ?? "this request"}</div>
        <div className="flex flex-col gap-1 font-mono text-[12.5px]">
          {d?.files.length ? d.files.map((f) => <div key={f} className="break-all">{f}</div>) : <span className="text-faint">None yet</span>}
        </div>
      </div>
      {prd.modules.length ? (
        <div className="text-[13.5px]">
          Modules to upgrade: <b className="text-ink">{prd.modules.join(", ")}</b>
        </div>
      ) : null}
      {d?.diff ? (
        <div className="overflow-x-auto rounded-[10px] bg-sunken font-mono text-[12.5px]">
          <pre className="min-w-max py-2">
            {d.diff.split("\n").map((l, i) => (
              <span key={i} className={cn("block px-3.5 whitespace-pre", l.startsWith("+") && !l.startsWith("+++") && "bg-[var(--add,#eaf5ee)]", l.startsWith("-") && !l.startsWith("---") && "bg-[var(--del,#fbeded)]", l.startsWith("@@") && "text-faint")}>
                {l || " "}
              </span>
            ))}
          </pre>
        </div>
      ) : null}
    </div>
  );
}

function HistoryTab({ prd }: { prd: Prd }) {
  return (
    <div className="flex flex-col gap-5.5">
      <Timeline entries={prd.timeline} live={prd.status === "ai"} />
      <details>
        <summary className="cursor-pointer text-[13.5px] font-semibold text-muted">AI cost</summary>
        <dl className="mt-3 grid grid-cols-[max-content_minmax(0,1fr)] gap-x-4.5 gap-y-1.5 text-[13.5px]">
          <dt className="text-faint">Total</dt>
          <dd>${prd.cost_usd.toFixed(2)}</dd>
          <dt className="text-faint">Tokens</dt>
          <dd>
            {prd.tokens_in.toLocaleString()} in · {prd.tokens_out.toLocaleString()} out
          </dd>
        </dl>
      </details>
      <details>
        <summary className="cursor-pointer text-[13.5px] font-semibold text-muted">In the brain</summary>
        <dl className="mt-3 grid grid-cols-[max-content_minmax(0,1fr)] gap-x-4.5 gap-y-1.5 font-mono text-[13px]">
          <dt className="font-sans text-faint">Request</dt>
          <dd>requirements/{prd.id}.md</dd>
          <dt className="font-sans text-faint">PRD</dt>
          <dd>prd/{prd.id}.md</dd>
          <dt className="font-sans text-faint">Decisions</dt>
          <dd>decisions/{prd.id}.md</dd>
          <dt className="font-sans text-faint">Changes</dt>
          <dd>changes/{prd.id}.md</dd>
        </dl>
      </details>
      {prd.context?.summary ? (
        <details>
          <summary className="cursor-pointer text-[13.5px] font-semibold text-muted">What the AI found in the code</summary>
          <p className="mt-3 text-[13.5px]">{prd.context.summary}</p>
        </details>
      ) : null}
    </div>
  );
}
