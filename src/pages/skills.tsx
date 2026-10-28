import { Plus, X } from "lucide-react";
import { useEffect, useState } from "react";

import { Page, PageHeader } from "@/components/page";
import { Button } from "@/components/ui/button";
import { Field, Input } from "@/components/ui/field";
import { Select, SelectContent, SelectItem, SelectTrigger } from "@/components/ui/select";
import { api } from "@/lib/api";
import { useAction, useStore } from "@/lib/store";
import type { ScanReport, Skill } from "@/lib/types";

const STEPS = [
  { key: "prd", label: "Writing the PRD" },
  { key: "develop", label: "Building" },
  { key: "test", label: "Testing" },
  { key: "commit", label: "Commit message" },
] as const;

export function SkillsPage() {
  const { wsId, workspace, refreshWorkspace } = useStore();
  const { run, busy } = useAction();
  const [skills, setSkills] = useState<Skill[]>([]);
  const [source, setSource] = useState("");
  const [result, setResult] = useState<{ name: string; installed: boolean; report: ScanReport } | null>(null);

  const load = () => void api.skills(wsId ?? undefined).then(setSkills).catch(() => setSkills([]));
  useEffect(load, [wsId]);

  if (!wsId || !workspace) return <Page>{null}</Page>;
  const steps = workspace.config.team.steps;

  const setStep = async (step: string, list: string[]) => {
    await run(() => api.setStepSkills(wsId, step, list));
    await refreshWorkspace();
  };

  const install = async (force: boolean) => {
    const r = await run(() => api.installSkill(source.trim(), force));
    if (r) {
      setResult(r);
      if (r.installed) {
        setSource("");
        load();
      }
    }
  };

  return (
    <Page narrow>
      <PageHeader title="Skills" sub="Which Claude Code skills the AI uses at each step. The team shares this list through the brain." />
      <div className="border-t border-line">
        {STEPS.map((s) => {
          const has = steps[s.key].skills;
          const rest = skills.filter((k) => !has.includes(k.name));
          return (
            <div key={s.key} className="grid grid-cols-[150px_minmax(0,1fr)] items-center gap-4 border-b border-line px-1 py-3.5 max-md:grid-cols-1">
              <div className="font-semibold text-ink">{s.label}</div>
              <div className="flex flex-wrap items-center gap-1.5">
                {has.map((k) => (
                  <span key={k} className="inline-flex h-7 items-center gap-1.5 rounded-full bg-sunken px-2.5 text-[13px] text-ink">
                    {k}
                    {!skills.some((x) => x.name === k) ? <span className="text-faint">(not installed)</span> : null}
                    <button aria-label={`Remove ${k}`} disabled={busy} className="cursor-pointer text-faint hover:text-bad" onClick={() => setStep(s.key, has.filter((x) => x !== k))}>
                      <X className="size-3" />
                    </button>
                  </span>
                ))}
                {rest.length ? (
                  <Select value="" onValueChange={(v) => setStep(s.key, [...has, v])}>
                    <SelectTrigger aria-label={`Add skill to ${s.label}`} className="h-7 w-auto rounded-full border-dashed bg-transparent px-2.5 text-[13px] font-semibold text-muted">
                      <span className="inline-flex items-center gap-1">
                        <Plus className="size-3" />
                        Add
                      </span>
                    </SelectTrigger>
                    <SelectContent>
                      {rest.map((k) => (
                        <SelectItem key={k.name} value={k.name} description={k.description}>
                          {k.name}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                ) : null}
              </div>
            </div>
          );
        })}
      </div>

      <Field label="Install a new skill" htmlFor="sk" hint="A git link or a folder with a SKILL.md. Every new skill gets a safety scan first.">
        <div className="flex gap-2.5">
          <Input id="sk" value={source} onChange={(e) => setSource(e.target.value)} placeholder="https://github.com/someone/odoo-skill" />
          <Button disabled={!source.trim() || busy} onClick={() => install(false)}>
            {busy ? "Working…" : "Install"}
          </Button>
        </div>
      </Field>
      {result ? (
        <div className="flex flex-col gap-3 rounded-[10px] border border-line bg-surface p-4">
          <p className="text-ink">
            <b>{result.name}</b> {result.installed ? "is installed." : "was not installed. The safety scan found serious problems:"}
          </p>
          {result.report.findings.length ? (
            <ul className="list-disc pl-5 text-[13.5px]">
              {result.report.findings.slice(0, 8).map((f, i) => (
                <li key={i}>
                  <b className={f.severity === "high" ? "text-bad" : "text-ink"}>{f.severity}</b> {f.message}
                  {f.file ? <span className="text-faint"> · {f.file}</span> : null}
                </li>
              ))}
            </ul>
          ) : (
            <p className="text-[13.5px] text-muted">No findings ({result.report.scanner} scan).</p>
          )}
          {!result.installed ? (
            <div className="flex gap-4">
              <Button size="sm" onClick={() => setResult(null)}>
                OK
              </Button>
              <Button variant="link" className="hover:text-bad" disabled={busy} onClick={() => install(true)}>
                Install anyway
              </Button>
            </div>
          ) : null}
        </div>
      ) : null}

      <div>
        <div className="mb-2.5 text-[13px] font-semibold text-muted">Installed on this computer</div>
        <div className="border-t border-line">
          {skills.length === 0 ? <p className="py-3 text-[13.5px] text-faint">No skills found in ~/.claude/skills or this project.</p> : null}
          {skills.map((s) => (
            <div key={s.name} className="border-b border-line px-1 py-3">
              <div className="font-semibold text-ink">{s.name}</div>
              <div className="text-[13.5px] text-muted">{s.description}</div>
              <div className="text-[12.5px] text-faint">{s.location}</div>
            </div>
          ))}
        </div>
      </div>
    </Page>
  );
}
