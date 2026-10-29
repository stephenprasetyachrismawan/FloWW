import { ArrowUp, ChevronRight, Square } from "lucide-react";
import { memo, useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import ReactMarkdown from "react-markdown";
import rehypeHighlight from "rehype-highlight";
import remarkGfm from "remark-gfm";

import { Button } from "@/components/ui/button";
import { api, openExternal } from "@/lib/api";
import { useAction, useStore } from "@/lib/store";
import type { ChatMsg, Prd } from "@/lib/types";
import { cn } from "@/lib/utils";

type Block = { kind: "msg"; msg: ChatMsg; key: string } | { kind: "tools"; items: ChatMsg[]; key: string };

/** Consecutive tool calls become one activity block, like the Claude Code extension. */
function blocks(chat: ChatMsg[]): Block[] {
  const out: Block[] = [];
  chat.forEach((m, i) => {
    const last = out[out.length - 1];
    if (m.role === "tool" && last?.kind === "tools") last.items.push(m);
    else if (m.role === "tool") out.push({ kind: "tools", items: [m], key: `t${i}` });
    else out.push({ kind: "msg", msg: m, key: `m${i}` });
  });
  return out;
}

/** The whole reply is one fenced JSON block (grill questions, PRD data). */
function onlyJson(text: string): boolean {
  const t = text.trim();
  return t.startsWith("```json") && t.endsWith("```") && t.indexOf("```", 7) === t.length - 3;
}

const Markdown = memo(function Markdown({ text }: { text: string }) {
  return (
    <div className="md">
      <ReactMarkdown
        remarkPlugins={[remarkGfm]}
        rehypePlugins={[[rehypeHighlight, { detect: true, ignoreMissing: true }]]}
        components={{
          a: ({ href, children }) => (
            <a
              href={href}
              onClick={(e) => {
                e.preventDefault();
                if (href && /^https?:/.test(href)) void openExternal(href);
              }}
            >
              {children}
            </a>
          ),
        }}
      >
        {text}
      </ReactMarkdown>
    </div>
  );
});

function Toggle({ open, onClick, children }: { open: boolean; onClick: () => void; children: React.ReactNode }) {
  return (
    <button type="button" onClick={onClick} className="inline-flex cursor-pointer items-center gap-1 text-[12.5px] text-faint hover:text-ink">
      <ChevronRight className={cn("size-3.5 transition-transform", open && "rotate-90")} />
      {children}
    </button>
  );
}

function FlowwRow({ msg }: { msg: ChatMsg }) {
  const [open, setOpen] = useState(false);
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex items-center gap-2 text-[12.5px] text-faint">
        <span className="h-px flex-1 bg-line" />
        <span>
          FloWW → Claude Code: <span className="text-muted">{msg.text}</span>
        </span>
        <span className="h-px flex-1 bg-line" />
      </div>
      {msg.detail ? (
        <div className="flex flex-col items-center gap-1.5">
          <Toggle open={open} onClick={() => setOpen(!open)}>
            {open ? "Hide what FloWW sent" : "Show what FloWW sent"}
          </Toggle>
          {open ? <pre className="max-h-80 w-full overflow-auto rounded-lg border border-line bg-sunken p-3 font-mono text-[12px] leading-relaxed whitespace-pre-wrap text-muted">{msg.detail}</pre> : null}
        </div>
      ) : null}
    </div>
  );
}

function AiRow({ msg }: { msg: ChatMsg }) {
  const [open, setOpen] = useState(false);
  if (onlyJson(msg.text)) {
    return (
      <div className="flex flex-col gap-1.5">
        <Toggle open={open} onClick={() => setOpen(!open)}>
          Structured answer for FloWW
        </Toggle>
        {open ? <Markdown text={msg.text} /> : null}
      </div>
    );
  }
  return <Markdown text={msg.text} />;
}

function YouRow({ msg }: { msg: ChatMsg }) {
  return (
    <div className="flex justify-end">
      <div className="max-w-[85%] rounded-[14px] rounded-br-[5px] bg-sunken px-3.5 py-2 whitespace-pre-wrap text-ink">{msg.text}</div>
    </div>
  );
}

const TOOL_VERB: Record<string, string> = {
  Read: "Read",
  Edit: "Edit",
  MultiEdit: "Edit",
  Write: "Write",
  Bash: "Run",
  Grep: "Search",
  Glob: "Find",
  TodoWrite: "Plan",
  WebFetch: "Fetch",
  WebSearch: "Search web",
  Task: "Agent",
};

function ToolsRow({ items, live }: { items: ChatMsg[]; live: boolean }) {
  const long = items.length > 4;
  const [open, setOpen] = useState(false);
  const shown = long && !open ? items.slice(-3) : items;
  return (
    <div className="flex flex-col gap-1 border-l-2 border-line pl-3">
      {long ? (
        <Toggle open={open} onClick={() => setOpen(!open)}>
          {open ? "Show fewer steps" : `${items.length - 3} earlier steps`}
        </Toggle>
      ) : null}
      {shown.map((t, i) => {
        const isLast = t === items[items.length - 1];
        return (
          <div key={i} className="flex min-w-0 items-baseline gap-2 text-[13px]">
            <i className={cn("relative top-[-1px] size-[6px] shrink-0 rounded-full bg-line-2", isLast && live && "bg-ai animate-[breathe_1.8s_ease-in-out_infinite]")} />
            <span className="shrink-0 font-semibold text-ink">{TOOL_VERB[t.tool ?? ""] ?? t.tool}</span>
            <span className="truncate font-mono text-[12px] text-muted" title={t.text}>
              {t.text}
            </span>
          </div>
        );
      })}
    </div>
  );
}

const STUCK_TEXT: Record<string, string> = {
  failed: "Stopped.",
  paused: "Paused.",
};

/**
 * The conversation with Claude Code for one request, with a box to add an
 * instruction at any time. While the AI works, sending stops it and it
 * continues the same session with your message.
 */
export function Chat({ prd, ws, placeholder }: { prd: Prd; ws: string; placeholder?: string }) {
  const { workspace } = useStore();
  const { run, busy } = useAction();
  const [text, setText] = useState("");
  const list = useRef<HTMLDivElement>(null);
  const stick = useRef(true);
  const live = prd.status === "ai" || prd.status === "wait";
  const items = useMemo(() => blocks(prd.chat ?? []), [prd.chat]);
  const stuck = prd.status === "failed" || prd.status === "paused" || (prd.status === "needs" && prd.step === "develop");
  const canTalk = prd.status !== "done";

  useLayoutEffect(() => {
    const el = list.current;
    if (el && stick.current) el.scrollTop = el.scrollHeight;
  }, [items.length, prd.chat?.length, live]);

  useEffect(() => {
    const el = list.current;
    if (!el) return;
    const onScroll = () => {
      stick.current = el.scrollHeight - el.scrollTop - el.clientHeight < 80;
    };
    el.addEventListener("scroll", onScroll);
    return () => el.removeEventListener("scroll", onScroll);
  }, []);

  async function send() {
    const t = text.trim();
    if (!t) return;
    stick.current = true;
    if (await run(() => api.act(ws, prd.id, "message", { text: t }))) setText("");
  }

  const statusLine = prd.status === "wait" && workspace?.lock.holder ? `Waiting for the database (${workspace.lock.holder} is using it)` : prd.sub;

  return (
    <div className="flex flex-col overflow-hidden rounded-[12px] border border-line bg-surface">
      <div ref={list} className="flex max-h-[62vh] min-h-[220px] flex-col gap-4 overflow-y-auto px-5 py-4.5">
        {items.length === 0 ? <p className="m-auto text-[13.5px] text-faint">The conversation with Claude Code shows up here.</p> : null}
        {items.map((b, i) =>
          b.kind === "tools" ? (
            <ToolsRow key={b.key} items={b.items} live={live && i === items.length - 1} />
          ) : b.msg.role === "floww" ? (
            <FlowwRow key={b.key} msg={b.msg} />
          ) : b.msg.role === "you" ? (
            <YouRow key={b.key} msg={b.msg} />
          ) : (
            <AiRow key={b.key} msg={b.msg} />
          ),
        )}
        {live ? (
          <div className="flex items-center gap-2 text-[13px] text-muted">
            <i className="size-[7px] rounded-full bg-ai animate-[breathe_1.8s_ease-in-out_infinite]" />
            {statusLine || "Working"}…
          </div>
        ) : null}
        {stuck ? (
          <div className="flex flex-wrap items-center gap-3 rounded-[10px] bg-sunken px-3.5 py-2.5 text-[13.5px]">
            <i className={cn("size-[7px] rounded-full", prd.status === "failed" ? "bg-bad" : "bg-you")} />
            <span className="text-ink">{STUCK_TEXT[prd.status] ?? "The tests still fail."}</span>
            <span className="text-muted">{prd.status === "failed" ? prd.sub : prd.status === "needs" ? "Tell Claude what to try below, or continue as is." : "Continue when you are ready."}</span>
            <span className="flex-1" />
            <Button size="sm" disabled={busy} onClick={() => run(() => api.act(ws, prd.id, "resume"))}>
              {prd.action === "Try again" ? "Try again" : "Continue"}
            </Button>
          </div>
        ) : null}
      </div>
      {canTalk ? (
        <div className="border-t border-line p-3">
          <div className="flex items-end gap-2 rounded-[10px] border border-line-2 bg-window px-3 py-2 focus-within:border-ink">
            <textarea
              aria-label="Message to Claude Code"
              rows={1}
              value={text}
              onChange={(e) => {
                setText(e.target.value);
                e.target.style.height = "auto";
                e.target.style.height = `${Math.min(e.target.scrollHeight, 180)}px`;
              }}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey && !e.nativeEvent.isComposing) {
                  e.preventDefault();
                  void send();
                }
              }}
              placeholder={live ? "Add an instruction. Claude stops and continues with it." : (placeholder ?? "Tell Claude what to change or add…")}
              className="max-h-[180px] min-h-[24px] flex-1 resize-none bg-transparent leading-normal text-ink outline-none placeholder:text-faint"
            />
            {prd.status === "ai" ? (
              <button
                type="button"
                aria-label="Stop"
                title="Stop"
                onClick={() => run(() => api.act(ws, prd.id, "pause"))}
                className="grid size-8 shrink-0 cursor-pointer place-items-center rounded-lg border border-line-2 text-muted hover:text-ink"
              >
                <Square className="size-3.5 fill-current" />
              </button>
            ) : null}
            <button
              type="button"
              aria-label="Send"
              title="Send (Enter)"
              disabled={!text.trim() || busy}
              onClick={send}
              className="grid size-8 shrink-0 cursor-pointer place-items-center rounded-lg bg-ink text-window disabled:cursor-default disabled:opacity-30"
            >
              <ArrowUp className="size-4" />
            </button>
          </div>
          <div className="mt-1.5 px-1 text-[12px] text-faint">Enter to send · Shift+Enter for a new line</div>
        </div>
      ) : null}
    </div>
  );
}
