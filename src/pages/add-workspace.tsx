import { Check, FolderOpen, LoaderCircle, X } from "lucide-react";
import { useState } from "react";

import { BranchTemplateField, DEFAULT_BRANCH_TEMPLATE } from "@/components/branch-template";
import { FolderBrowser } from "@/components/folder-browser";
import { Button } from "@/components/ui/button";
import { Field, Input, OptionList } from "@/components/ui/field";
import { Modal } from "@/components/ui/modal";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { api, inTauri, pickFolderNative } from "@/lib/api";
import { useAction, useStore } from "@/lib/store";
import type { Detection } from "@/lib/types";

/** Pick a repo, let FloWW look at it, answer what is unclear, review, create. */
export function AddWorkspace({ onClose }: { onClose: () => void }) {
  const { setWsId, refreshApp } = useStore();
  const { run, busy } = useAction();
  const [path, setPath] = useState("");
  const [det, setDet] = useState<Detection | null>(null);
  const [detecting, setDetecting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [browsing, setBrowsing] = useState(false);
  const [browsePath, setBrowsePath] = useState("");
  const [answers, setAnswers] = useState<Record<string, string>>({});
  const [form, setForm] = useState({ name: "", initials: "", db: "", base_branch: "preprod", branch_template: DEFAULT_BRANCH_TEMPLATE, launch_config: "", http_port: 8069, language: "id" });
  const [templateOk, setTemplateOk] = useState(true);

  async function look(target = path) {
    setPath(target);
    setBrowsing(false);
    setDetecting(true);
    setError(null);
    try {
      const d = await api.detect(target);
      setDet(d);
      setAnswers({});
      setForm({
        name: d.name,
        initials: d.initials,
        db: d.suggested.db,
        base_branch: d.suggested.base_branch,
        branch_template: DEFAULT_BRANCH_TEMPLATE,
        launch_config: d.suggested.launch_config ?? "",
        http_port: d.suggested.http_port,
        language: "id",
      });
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setDetecting(false);
    }
  }

  async function choose() {
    if (inTauri) {
      try {
        const picked = await pickFolderNative("Choose the repository folder");
        if (picked) await look(picked);
        return;
      } catch {
        // No system picker available: use the built-in one.
      }
    }
    setBrowsing(true);
  }

  const suggestion = error?.match(/Did you mean (.+)\?/)?.[1];

  const open = det?.choices.find((c) => !(c.key in answers));
  const stage = !det ? "path" : open ? "ask" : "review";

  async function create() {
    const r = await run(() =>
      api.addWorkspace({
        path: det!.repo,
        name: form.name,
        initials: form.initials,
        db: form.db,
        base_branch: form.base_branch,
        branch_template: form.branch_template,
        launch_config: form.launch_config || null,
        http_port: Number(form.http_port) || 8069,
        language: form.language,
      }),
    );
    if (r) {
      await refreshApp();
      setWsId(r.id);
      onClose();
    }
  }

  const title = browsing ? "Choose the repository folder" : stage === "path" ? "Which project?" : stage === "ask" ? "One question" : "Look right?";
  const sub = browsing
    ? "Open folders until you reach the repo, then choose it."
    : stage === "path"
      ? "Each repository gets its own board, database and brain."
      : stage === "ask"
        ? "FloWW found more than one option."
        : "FloWW filled these in from your repo and computer. Change anything that looks off.";

  return (
    <Modal
      title={title}
      sub={sub}
      step={`Add workspace · Step ${stage === "path" ? 1 : stage === "ask" ? 2 : 3} of 3`}
      onClose={onClose}
      wide
      footer={
        browsing ? (
          <>
            <Button variant="link" onClick={() => setBrowsing(false)}>
              Back
            </Button>
            <span className="flex-1" />
            <Button variant="primary" onClick={() => look(browsePath)} disabled={!browsePath}>
              Use this folder
            </Button>
          </>
        ) : stage === "path" ? (
          <>
            <span className="flex-1" />
            <Button variant="link" onClick={onClose}>
              Cancel
            </Button>
            <Button variant="primary" onClick={() => look()} disabled={!path.trim() || detecting}>
              {detecting ? "Looking…" : "Look at this folder"}
            </Button>
          </>
        ) : stage === "review" ? (
          <>
            <Button variant="link" onClick={() => setDet(null)}>
              Back
            </Button>
            <span className="flex-1" />
            <Button variant="primary" onClick={create} disabled={busy || !det?.is_git || !form.initials || !form.base_branch.trim() || !templateOk}>
              {busy ? "Setting up…" : "Create workspace"}
            </Button>
          </>
        ) : null
      }
    >
      {browsing ? <FolderBrowser start={path.trim() || undefined} onChange={setBrowsePath} onPick={(p) => look(p)} /> : null}

      {!browsing && stage === "path" ? (
        <Field label="Repository folder" htmlFor="ws-path" hint={inTauri ? "Choose it, or paste the full path." : "Choose it, or paste the full path on the computer running FloWW."}>
          <div className="flex gap-2">
            <Input
              id="ws-path"
              value={path}
              onChange={(e) => setPath(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && path.trim() && look()}
              placeholder="/home/you/work/your-repo"
            />
            <Button onClick={choose} disabled={detecting}>
              <FolderOpen className="size-4" />
              Choose folder…
            </Button>
          </div>
          {error ? (
            <span className="flex flex-col items-start gap-1 text-[13px] text-bad">
              {error}
              {suggestion ? (
                <Button variant="link" className="text-[13px] text-ink underline" onClick={() => look(suggestion)}>
                  Use {suggestion}
                </Button>
              ) : null}
            </span>
          ) : null}
        </Field>
      ) : null}

      {det && stage === "ask" && open ? (
        <>
          <div className="font-serif text-xl leading-snug font-medium text-ink">{open.question}</div>
          <OptionList
            options={open.options.map((o) => ({ label: o.value, why: o.why, recommended: o.recommended }))}
            onPick={(v) => {
              setAnswers((a) => ({ ...a, [open.key]: v }));
              setForm((f) => ({ ...f, [open.key]: v }));
            }}
          />
        </>
      ) : null}

      {det && stage === "review" ? (
        <>
          <div className="text-[13.5px] text-muted">
            Repository <span className="font-mono text-ink">{det.repo}</span>
          </div>
          <ul className="grid grid-cols-2 gap-x-6 gap-y-1.5 text-[13.5px] max-md:grid-cols-1">
            {det.checks.map((c) => (
              <li key={c.label} className="flex items-start gap-2">
                {c.ok ? <Check className="mt-0.5 size-4 shrink-0 text-ok" /> : <X className="mt-0.5 size-4 shrink-0 text-faint" />}
                <span>
                  <span className="text-ink">{c.label}</span> <span className="text-faint">· {c.detail}</span>
                </span>
              </li>
            ))}
          </ul>
          {!det.is_git ? <p className="text-[13.5px] text-bad">This folder is not a git repository. Go back and choose the folder that has the .git folder, or run git init there first.</p> : null}
          <div className="grid grid-cols-2 gap-4 max-md:grid-cols-1">
            <Field label="Workspace name" htmlFor="f-name">
              <Input id="f-name" value={form.name} onChange={(e) => setForm({ ...form, name: e.target.value })} />
            </Field>
            <Field label="Your initials" htmlFor="f-ini" hint={`PRD ids look like ${form.initials || "XX"}-001`}>
              <Input id="f-ini" value={form.initials} maxLength={4} onChange={(e) => setForm({ ...form, initials: e.target.value.toUpperCase().replace(/[^A-Z0-9]/g, "") })} />
            </Field>
            <Field
              label="Development branch"
              htmlFor="f-base"
              hint={
                det.branches.includes(form.base_branch.trim())
                  ? "New branches start here and pull requests go here. Found in the repo."
                  : `New branches start here and pull requests go here. Will be created from ${det.default_branch ?? "the default branch"}.`
              }
            >
              <Input id="f-base" value={form.base_branch} onChange={(e) => setForm({ ...form, base_branch: e.target.value })} />
            </Field>
            <BranchTemplateField id="f-template" value={form.branch_template} onChange={(v) => setForm({ ...form, branch_template: v })} initials={form.initials} onValid={setTemplateOk} />
            <div className="flex flex-col gap-1.5">
              <span className="text-[13.5px] font-semibold text-ink">Local database</span>
              <Select value={form.db || undefined} onValueChange={(v) => setForm({ ...form, db: v })}>
                <SelectTrigger aria-label="Local database">
                  <SelectValue placeholder={det.databases.length ? "Choose" : "No databases found"} />
                </SelectTrigger>
                <SelectContent>
                  {det.databases.map((d) => (
                    <SelectItem key={d} value={d}>
                      {d}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div className="flex flex-col gap-1.5">
              <span className="text-[13.5px] font-semibold text-ink">Odoo launch config</span>
              <Select value={form.launch_config || undefined} onValueChange={(v) => setForm({ ...form, launch_config: v })}>
                <SelectTrigger aria-label="Odoo launch config">
                  <SelectValue placeholder={det.launches.length ? "Choose" : "None in .vscode/launch.json"} />
                </SelectTrigger>
                <SelectContent>
                  {det.launches.map((l) => (
                    <SelectItem key={l.name} value={l.name} description={l.config_file ?? undefined}>
                      {l.name}
                    </SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <Field label="Odoo port" htmlFor="f-port">
              <Input id="f-port" type="number" value={form.http_port} onChange={(e) => setForm({ ...form, http_port: Number(e.target.value) })} />
            </Field>
            <div className="flex flex-col gap-1.5">
              <span className="text-[13.5px] font-semibold text-ink">AI writes in</span>
              <Select value={form.language} onValueChange={(v) => setForm({ ...form, language: v })}>
                <SelectTrigger aria-label="AI language">
                  <SelectValue />
                </SelectTrigger>
                <SelectContent>
                  <SelectItem value="id">Bahasa Indonesia</SelectItem>
                  <SelectItem value="en">English</SelectItem>
                </SelectContent>
              </Select>
            </div>
          </div>
          {busy ? (
            <p className="flex items-center gap-2 text-[13.5px] text-muted">
              <LoaderCircle className="size-4 animate-spin" />
              Creating the brain, reading modules, preparing branches…
            </p>
          ) : null}
        </>
      ) : null}
    </Modal>
  );
}
