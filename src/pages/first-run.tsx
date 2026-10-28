import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { Field, Input, OptionList } from "@/components/ui/field";
import { api } from "@/lib/api";
import { useAction, useStore } from "@/lib/store";
import { cn } from "@/lib/utils";

const NOTES: Record<string, string> = {
  "claude-opus-5-5": "Strongest, slower",
  "claude-sonnet-5-5": "Balanced",
  "claude-haiku-5-5": "Fast and cheap",
  "claude-fable-5-1": "Claude Code alias",
};

/** Shown until AI access is set. Two short steps. */
export function FirstRun() {
  const { refreshApp } = useStore();
  const { run, busy } = useAction();
  const [step, setStep] = useState<0 | 1>(0);
  const [mode, setMode] = useState<"subscription" | "router">("subscription");
  const [url, setUrl] = useState("http://localhost:20128");
  const [token, setToken] = useState("");
  const [models, setModels] = useState<string[]>([]);
  const [model, setModel] = useState("claude-sonnet-5-5");
  const [test, setTest] = useState<string | null>(null);
  const [testing, setTesting] = useState(false);

  useEffect(() => {
    if (step === 1) void api.models().then(setModels).catch(() => setModels(Object.keys(NOTES)));
  }, [step]);

  async function saveAuth() {
    const ok = await run(() => api.setAi({ mode, url: mode === "router" ? url : undefined, token: token || undefined }));
    if (ok !== undefined) setStep(1);
  }

  async function sendTest() {
    setTesting(true);
    setTest(null);
    try {
      const r = await api.testAi(model);
      setTest(r.message);
    } catch (e) {
      setTest(`Did not work: ${(e as Error).message}`);
    } finally {
      setTesting(false);
    }
  }

  async function finish() {
    const ok = await run(() => api.setAi({ mode, url: mode === "router" ? url : undefined, model }));
    if (ok !== undefined) await refreshApp();
  }

  return (
    <div className="mx-auto flex w-full max-w-[620px] flex-col gap-6 px-6 py-14">
      <div>
        <div className="text-[12.5px] text-faint">Welcome · Step {step + 1} of 2</div>
        <h1 className="mt-1 font-serif text-[28px] leading-tight font-semibold">{step === 0 ? "How should FloWW talk to Claude?" : "Which model should it use?"}</h1>
        <p className="mt-1.5 text-muted">
          {step === 0 ? "FloWW runs every AI step through Claude Code on this computer." : "It also sets up your projects. You can change it per step later."}
        </p>
      </div>

      {step === 0 ? (
        <>
          <div className="flex flex-col border-t border-line">
            {[
              { v: "subscription" as const, l: "My Claude subscription", w: "Uses the login already in Claude Code (claude login)." },
              { v: "router" as const, l: "9router", w: "Routes Claude Code through your 9router with its API key." },
            ].map((o) => (
              <button
                key={o.v}
                type="button"
                onClick={() => setMode(o.v)}
                className="grid w-full cursor-pointer grid-cols-[18px_minmax(0,1fr)] gap-3.5 border-b border-line px-1 py-3.5 text-left"
              >
                <span className={cn("mt-[3px] grid size-4 place-items-center rounded-full border-[1.5px] border-line-2", mode === o.v && "border-accent")}>
                  {mode === o.v ? <span className="size-2 rounded-full bg-accent" /> : null}
                </span>
                <span>
                  <span className="font-semibold text-ink">{o.l}</span>
                  <span className="block text-[13.5px] text-muted">{o.w}</span>
                </span>
              </button>
            ))}
          </div>
          {mode === "router" ? (
            <div className="grid grid-cols-2 gap-4.5 max-md:grid-cols-1">
              <Field label="9router address" htmlFor="r-url" hint="Without /v1.">
                <Input id="r-url" value={url} onChange={(e) => setUrl(e.target.value)} />
              </Field>
              <Field label="API key" htmlFor="r-key" hint="Saved in your system keychain.">
                <Input id="r-key" type="password" value={token} onChange={(e) => setToken(e.target.value)} />
              </Field>
            </div>
          ) : null}
          <div className="flex justify-end">
            <Button variant="primary" onClick={saveAuth} disabled={busy}>
              Continue
            </Button>
          </div>
        </>
      ) : (
        <>
          <OptionList
            options={(models.length ? models : Object.keys(NOTES)).map((m) => ({ label: m, why: NOTES[m] ?? (mode === "router" ? "via 9router" : ""), recommended: m === "claude-sonnet-5-5" }))}
            onPick={(m) => {
              setModel(m);
              setTest(null);
            }}
          />
          <div className="text-[13.5px]">
            Selected: <b className="font-mono font-medium text-ink">{model}</b>
          </div>
          <div className="flex flex-wrap items-center gap-4">
            <Button variant="link" onClick={sendTest} disabled={testing}>
              {testing ? "Testing…" : "Send a test message"}
            </Button>
            {test ? <span className="text-[13.5px] text-muted">{test}</span> : null}
            <span className="flex-1" />
            <Button variant="link" onClick={() => setStep(0)}>
              Back
            </Button>
            <Button variant="primary" onClick={finish} disabled={busy}>
              Done
            </Button>
          </div>
        </>
      )}
    </div>
  );
}
