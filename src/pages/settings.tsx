import { useEffect, useState } from "react";

import { Page, PageHeader } from "@/components/page";
import { Button } from "@/components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { inTauri, launchConfigs, loadWorkspace } from "@/lib/api";
import type { EffectiveConfig, OdooLaunch } from "@/lib/types";

const MODELS = [
  { id: "claude-opus-5-5", note: "Strongest, slower" },
  { id: "claude-sonnet-5-5", note: "Balanced" },
  { id: "claude-haiku-5-5", note: "Fast and cheap" },
];
const REPO_KEY = "floww.repo";

function readRepo(): string {
  try {
    return localStorage.getItem(REPO_KEY) ?? "";
  } catch {
    return "";
  }
}

export function SettingsPage() {
  const [repo, setRepo] = useState(readRepo);
  const [cfg, setCfg] = useState<EffectiveConfig | null>(null);
  const [launches, setLaunches] = useState<OdooLaunch[]>([]);
  const [error, setError] = useState<string | null>(null);

  async function load(path: string) {
    setError(null);
    try {
      const [c, l] = await Promise.all([loadWorkspace(path), launchConfigs(path)]);
      setCfg(c);
      setLaunches(l);
      try {
        localStorage.setItem(REPO_KEY, path);
      } catch {
        // Remembering the folder is a convenience only.
      }
    } catch (e) {
      setError(String(e));
    }
  }

  useEffect(() => {
    if (repo || !inTauri) void load(repo);
    // Load once when the page opens.
  }, []);

  return (
    <Page narrow>
      <PageHeader title="Settings" sub={inTauri ? undefined : "Browser preview: showing example settings."} />

      <div className="flex flex-col gap-1.5">
        <label htmlFor="repo" className="text-[13.5px] font-semibold text-ink">
          Repository folder
        </label>
        <div className="flex gap-2.5">
          <input
            id="repo"
            className="h-[38px] min-w-0 flex-1 rounded-lg border border-line-2 bg-surface px-3 text-ink outline-none focus:border-ink"
            placeholder="/home/you/work/acme-odoo"
            value={repo}
            onChange={(e) => setRepo(e.target.value)}
          />
          <Button onClick={() => void load(repo)} disabled={inTauri && !repo.trim()}>
            Load
          </Button>
        </div>
        {error ? <p className="text-[13px] text-bad">{error}</p> : null}
      </div>

      {cfg ? (
        <>
          <section className="flex flex-col gap-4">
            <div>
              <h3 className="text-base font-semibold">For the whole team</h3>
              <p className="text-[13.5px] text-muted">From brain/_floww/team.toml.</p>
            </div>
            <dl className="grid grid-cols-[max-content_minmax(0,1fr)] gap-x-5 gap-y-1.5 text-[13.5px]">
              <dt className="text-faint">New work starts from</dt>
              <dd>{cfg.team.base_branch}</dd>
              <dt className="text-faint">Branch name</dt>
              <dd className="font-mono">{cfg.team.feature_branch}</dd>
              <dt className="text-faint">Building skills</dt>
              <dd>{cfg.team.steps.develop.skills.join(", ") || "None"}</dd>
              <dt className="text-faint">AI tries before asking</dt>
              <dd>{cfg.team.steps.develop.max_iterations} times</dd>
            </dl>
          </section>

          <section className="flex flex-col gap-4">
            <div>
              <h3 className="text-base font-semibold">On this computer</h3>
              <p className="text-[13.5px] text-muted">From .floww/workspace.toml. Only you.</p>
            </div>
            <div className="grid grid-cols-2 gap-4.5 max-md:grid-cols-1">
              <div className="flex flex-col gap-1.5">
                <span className="text-[13.5px] font-semibold text-ink">Odoo launch config</span>
                <Select
                  value={cfg.workspace.launch_config ?? undefined}
                  onValueChange={(v) => setCfg({ ...cfg, workspace: { ...cfg.workspace, launch_config: v } })}
                >
                  <SelectTrigger aria-label="Odoo launch config">
                    <SelectValue placeholder={launches.length ? "Choose" : "No odoo-bin config in launch.json"} />
                  </SelectTrigger>
                  <SelectContent>
                    {launches.map((l) => (
                      <SelectItem key={l.name} value={l.name} description={l.config_file ?? undefined}>
                        {l.name}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>
              <div className="flex flex-col gap-1.5">
                <span className="text-[13.5px] font-semibold text-ink">AI model</span>
                <Select
                  value={cfg.workspace.models.default ?? undefined}
                  onValueChange={(v) =>
                    setCfg({ ...cfg, workspace: { ...cfg.workspace, models: { ...cfg.workspace.models, default: v } } })
                  }
                >
                  <SelectTrigger aria-label="AI model">
                    <SelectValue placeholder="Choose" />
                  </SelectTrigger>
                  <SelectContent>
                    {MODELS.map((m) => (
                      <SelectItem key={m.id} value={m.id} description={m.note}>
                        {m.id}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <span className="text-[12.5px] text-faint">
                  {cfg.workspace.ai.mode === "router" ? `Through 9router at ${cfg.workspace.ai.url}` : "Signed in with your Claude subscription."}
                </span>
              </div>
            </div>
            <p className="text-[12.5px] text-faint">Saving settings comes with the workspace setup flow.</p>
          </section>
        </>
      ) : null}
    </Page>
  );
}
