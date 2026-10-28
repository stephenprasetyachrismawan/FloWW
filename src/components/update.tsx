import { RefreshCw } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Button } from "@/components/ui/button";
import { Modal } from "@/components/ui/modal";
import { api, inTauri, openExternal, startDesktopUpdate, subscribe } from "@/lib/api";
import type { UpdateCheck, UpdateProgress } from "@/lib/types";
import { cn } from "@/lib/utils";

const STEPS: { stage: UpdateProgress["stage"]; label: string }[] = [
  { stage: "download", label: "Download" },
  { stage: "verify", label: "Check the file" },
  { stage: "install", label: "Install" },
  { stage: "cleanup", label: "Remove old files" },
  { stage: "restart", label: "Open the new version" },
];

const ORDER: UpdateProgress["stage"][] = ["check", "download", "verify", "install", "cleanup", "restart"];

/** Steps with a dot each, a bar while downloading, and the current message. */
export function UpdateProgressView({ p }: { p: UpdateProgress }) {
  const at = ORDER.indexOf(p.stage);
  const failed = p.stage === "failed";
  return (
    <div className="flex flex-col gap-3">
      <ol className={cn("flex flex-col gap-1.5 text-[13.5px]", failed && "hidden")}>
        {STEPS.map((s) => {
          const i = ORDER.indexOf(s.stage);
          const done = !failed && i < at;
          const now = !failed && i === at;
          return (
            <li key={s.stage} className={cn("flex items-center gap-2.5", done || now ? "text-ink" : "text-faint")}>
              <i
                className={cn(
                  "size-[7px] shrink-0 rounded-full bg-line-2",
                  done && "bg-ok",
                  now && "bg-ai animate-[breathe_1.8s_ease-in-out_infinite]",
                )}
              />
              {s.label}
            </li>
          );
        })}
      </ol>
      {p.stage === "download" && p.percent !== null ? (
        <div className="h-1.5 w-full overflow-hidden rounded-full bg-sunken" role="progressbar" aria-valuenow={p.percent} aria-valuemin={0} aria-valuemax={100}>
          <div className="h-full bg-ai transition-[width]" style={{ width: `${p.percent}%` }} />
        </div>
      ) : null}
      <p className={cn("text-[13.5px]", failed ? "text-bad" : "text-muted")}>{p.message}</p>
    </div>
  );
}

/** Sidebar: always there. Checks GitHub when clicked. */
export function UpdateControl({ version }: { version: string }) {
  const [checking, setChecking] = useState(false);
  const [note, setNote] = useState<{ ok: boolean; text: string } | null>(null);
  const [found, setFound] = useState<UpdateCheck | null>(null);
  const timer = useRef<number | undefined>(undefined);

  async function check() {
    setChecking(true);
    setNote(null);
    window.clearTimeout(timer.current);
    try {
      const c = await api.updateCheck();
      if (c.newer) setFound(c);
      else {
        setNote({ ok: true, text: `You have the latest version (${c.current}).` });
        timer.current = window.setTimeout(() => setNote(null), 10_000);
      }
    } catch (e) {
      setNote({ ok: false, text: (e as Error).message });
    } finally {
      setChecking(false);
    }
  }

  useEffect(() => () => window.clearTimeout(timer.current), []);

  return (
    <>
      <button
        type="button"
        onClick={check}
        disabled={checking}
        className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-[7px] text-faint hover:bg-sunken hover:text-ink disabled:cursor-default"
      >
        <RefreshCw className={cn("size-[15px]", checking && "animate-spin")} strokeWidth={1.8} />
        {checking ? "Checking…" : "Check for updates"}
      </button>
      {note ? (
        <span role="status" className={cn("flex items-start gap-2 px-2.5 py-1 text-[12.5px] leading-snug", note.ok ? "text-muted" : "text-bad")}>
          <i className={cn("mt-[5px] size-[7px] shrink-0 rounded-full", note.ok ? "bg-ok" : "bg-bad")} />
          {note.text}
        </span>
      ) : null}
      <span className="px-2.5 py-1 text-faint">FloWW {version}</span>
      {found ? <UpdateModal check={found} onClose={() => setFound(null)} /> : null}
    </>
  );
}

function UpdateModal({ check, onClose }: { check: UpdateCheck; onClose: () => void }) {
  const [progress, setProgress] = useState<UpdateProgress | null>(null);
  const running = progress !== null && progress.stage !== "failed";

  async function start() {
    if (inTauri) {
      setProgress({ stage: "check", percent: null, message: "Closing FloWW to update it…" });
      try {
        await startDesktopUpdate();
      } catch (e) {
        setProgress({ stage: "failed", percent: null, message: (e as Error).message });
      }
      return;
    }
    setProgress({ stage: "check", percent: null, message: "Starting…" });
    let restarting = false;
    const stop = subscribe((e) => {
      if (e.type !== "update") return;
      setProgress({ stage: e.stage, percent: e.percent, message: e.message });
      if (e.stage === "restart" && !restarting) {
        restarting = true;
        void waitForRestart(check.latest).then((ok) => {
          stop();
          if (ok) window.location.reload();
          else setProgress({ stage: "failed", percent: null, message: "FloWW did not come back after the update. Start floww-web again." });
        });
      }
    });
    try {
      await api.updateApply();
    } catch (e) {
      stop();
      setProgress({ stage: "failed", percent: null, message: (e as Error).message });
    }
  }

  const what =
    check.kind === "web"
      ? "FloWW downloads the new version, restarts itself, and this page reloads."
      : check.kind === "deb"
        ? "FloWW closes, a small window shows the progress and asks for your password to install, then the new version opens."
        : "FloWW closes, a small window shows the progress, then the new version opens.";

  return (
    <Modal
      title={`FloWW ${check.latest} is ready`}
      sub={`You have ${check.current}.`}
      onClose={running ? () => {} : onClose}
      footer={
        progress ? (
          progress.stage === "failed" ? (
            <>
              <span className="flex-1" />
              <Button variant="link" onClick={() => void openExternal(check.release_url)}>
                Release page
              </Button>
              <Button onClick={onClose}>Close</Button>
            </>
          ) : null
        ) : (
          <>
            <Button variant="link" onClick={() => void openExternal(check.release_url)}>
              What's new
            </Button>
            <span className="flex-1" />
            <Button variant="link" onClick={onClose}>
              Later
            </Button>
            {check.can_apply ? (
              <Button variant="primary" onClick={start}>
                Update now
              </Button>
            ) : (
              <Button variant="primary" onClick={() => void openExternal(check.release_url)}>
                Open release page
              </Button>
            )}
          </>
        )
      }
    >
      {progress ? (
        <UpdateProgressView p={progress} />
      ) : (
        <div className="flex flex-col gap-3 text-[14px]">
          <p className="text-ink">{what}</p>
          <p className="text-muted">Your workspaces, settings and brain stay as they are.</p>
          {check.old_files.length ? (
            <div className="flex flex-col gap-1.5">
              <span className="text-[13.5px] font-semibold text-ink">Old installer files that will be removed</span>
              <ul className="max-h-32 overflow-auto rounded-lg bg-sunken px-3 py-2 font-mono text-[12.5px] text-muted">
                {check.old_files.map((f) => (
                  <li key={f} className="truncate" title={f}>
                    {f}
                  </li>
                ))}
              </ul>
            </div>
          ) : null}
          {check.reason ? <p className="text-[13.5px] text-bad">{check.reason}</p> : null}
        </div>
      )}
    </Modal>
  );
}

/** After a web update the server restarts. Wait until it answers with the new version. */
async function waitForRestart(version: string): Promise<boolean> {
  const until = Date.now() + 90_000;
  while (Date.now() < until) {
    await new Promise((r) => setTimeout(r, 1000));
    try {
      const a = await api.app();
      if (a.version === version) return true;
    } catch {
      // Still restarting.
    }
  }
  return false;
}
