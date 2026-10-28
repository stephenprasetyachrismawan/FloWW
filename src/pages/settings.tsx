import { useEffect, useState } from "react";

import { BranchTemplateField, DEFAULT_BRANCH_TEMPLATE } from "@/components/branch-template";
import { Page, PageHeader } from "@/components/page";
import { Button } from "@/components/ui/button";
import { Field, Input } from "@/components/ui/field";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { api } from "@/lib/api";
import { useAction, useStore } from "@/lib/store";

const NOTES: Record<string, string> = { "claude-opus-5-5": "Strongest, slower", "claude-sonnet-5-5": "Balanced", "claude-haiku-5-5": "Fast and cheap" };

export function SettingsPage() {
  const { wsId, workspace, app, refreshWorkspace, refreshApp, setWsId } = useStore();
  const { run, busy } = useAction();
  const [models, setModels] = useState<string[]>(Object.keys(NOTES));
  const [f, setF] = useState<Record<string, string | number | boolean>>({});
  const [confirmRemove, setConfirmRemove] = useState(false);
  const [templateOk, setTemplateOk] = useState(true);

  useEffect(() => {
    void api.models().then(setModels).catch(() => undefined);
  }, []);
  useEffect(() => {
    if (!workspace) return;
    const c = workspace.config;
    setF({
      base_branch: c.team.base_branch,
      feature_branch: c.team.feature_branch || DEFAULT_BRANCH_TEMPLATE,
      language: c.team.language,
      full_suite: c.team.steps.full_suite,
      db: c.workspace.db,
      launch_config: c.workspace.launch_config ?? "",
      http_port: c.workspace.http_port,
      model: c.workspace.models.default ?? "",
      develop_model: c.workspace.models.develop ?? "default",
      github_token: "",
    });
  }, [workspace]);

  if (!wsId || !workspace) return <Page>{null}</Page>;
  const set = (k: string) => (v: string | number | boolean) => setF((x) => ({ ...x, [k]: v }));

  const save = async () => {
    await run(
      () =>
        api.saveSettings(wsId, {
          base_branch: f.base_branch,
          feature_branch: f.feature_branch,
          language: f.language,
          full_suite: f.full_suite,
          db: f.db,
          launch_config: f.launch_config || undefined,
          http_port: Number(f.http_port),
          model: f.model,
          develop_model: f.develop_model,
          github_token: f.github_token || undefined,
        }),
      "Settings saved",
    );
    await refreshWorkspace();
  };

  const modelItems = models.map((m) => (
    <SelectItem key={m} value={m} description={NOTES[m]}>
      {m}
    </SelectItem>
  ));

  return (
    <Page narrow>
      <PageHeader title="Settings" sub={workspace.path} />
      <section className="flex flex-col gap-4.5">
        <div>
          <h3 className="text-base font-semibold">For the whole team</h3>
          <p className="text-[13.5px] text-muted">Saved in the shared brain. Changes are recorded with your name.</p>
        </div>
        <div className="grid grid-cols-2 gap-4.5 max-md:grid-cols-1">
          <Field label="Development branch" htmlFor="s-base" hint="New branches start here and pull requests go here.">
            <Input id="s-base" value={String(f.base_branch ?? "")} onChange={(e) => set("base_branch")(e.target.value)} />
          </Field>
          <BranchTemplateField id="s-template" value={String(f.feature_branch ?? "")} onChange={set("feature_branch")} initials={workspace.config.workspace.initials} onValid={setTemplateOk} />
          <div className="flex flex-col gap-1.5">
            <span className="text-[13.5px] font-semibold text-ink">AI writes in</span>
            <Select value={String(f.language ?? "id")} onValueChange={set("language")}>
              <SelectTrigger aria-label="AI language">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="id">Bahasa Indonesia</SelectItem>
                <SelectItem value="en">English</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <label className="flex cursor-pointer items-center gap-2.5 self-end pb-2.5 text-text">
            <input type="checkbox" className="size-4 accent-[var(--accent)]" checked={Boolean(f.full_suite)} onChange={(e) => set("full_suite")(e.target.checked)} />
            Run the full test suite after building
          </label>
        </div>
      </section>

      <section className="flex flex-col gap-4.5">
        <div>
          <h3 className="text-base font-semibold">On this computer</h3>
          <p className="text-[13.5px] text-muted">Only you. Not shared.</p>
        </div>
        <div className="grid grid-cols-2 gap-4.5 max-md:grid-cols-1">
          <Field label="Local database" htmlFor="s-db">
            <Input id="s-db" value={String(f.db ?? "")} onChange={(e) => set("db")(e.target.value)} />
          </Field>
          <div className="flex flex-col gap-1.5">
            <span className="text-[13.5px] font-semibold text-ink">Odoo launch config</span>
            <Select value={String(f.launch_config || "") || undefined} onValueChange={set("launch_config")}>
              <SelectTrigger aria-label="Odoo launch config">
                <SelectValue placeholder={workspace.launches.length ? "Choose" : "None in .vscode/launch.json"} />
              </SelectTrigger>
              <SelectContent>
                {workspace.launches.map((l) => (
                  <SelectItem key={l.name} value={l.name} description={l.config_file ?? undefined}>
                    {l.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>
          <Field label="Odoo port" htmlFor="s-port">
            <Input id="s-port" type="number" value={Number(f.http_port ?? 8069)} onChange={(e) => set("http_port")(Number(e.target.value))} />
          </Field>
          <div className="flex flex-col gap-1.5">
            <span className="text-[13.5px] font-semibold text-ink">AI model</span>
            <Select value={String(f.model || app?.default_model || "") || undefined} onValueChange={set("model")}>
              <SelectTrigger aria-label="AI model">
                <SelectValue placeholder="App default" />
              </SelectTrigger>
              <SelectContent>{modelItems}</SelectContent>
            </Select>
            <span className="text-[12.5px] text-faint">{app?.ai?.mode === "router" ? `Through 9router at ${app.ai.url}` : "Signed in with your Claude subscription."}</span>
          </div>
          <div className="flex flex-col gap-1.5">
            <span className="text-[13.5px] font-semibold text-ink">Model for building</span>
            <Select value={String(f.develop_model ?? "default")} onValueChange={set("develop_model")}>
              <SelectTrigger aria-label="Model for building">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="default">Same as above</SelectItem>
                {modelItems}
              </SelectContent>
            </Select>
          </div>
          <Field label="GitHub token" htmlFor="s-gh" hint={workspace.config.workspace.github_token_ref ? "Saved in your keychain. Type a new one to replace it." : "For opening pull requests. Saved in your keychain. Or sign in to the gh CLI."}>
            <Input id="s-gh" type="password" value={String(f.github_token ?? "")} onChange={(e) => set("github_token")(e.target.value)} placeholder={workspace.config.workspace.github_token_ref ? "••••••••" : ""} />
          </Field>
        </div>
      </section>

      <div className="flex flex-wrap items-center gap-4.5">
        <Button variant="primary" disabled={busy || !templateOk} onClick={save}>
          Save settings
        </Button>
        <span className="flex-1" />
        {confirmRemove ? (
          <>
            <span className="text-[13.5px] text-muted">Remove from FloWW? Files and branches stay.</span>
            <Button
              size="sm"
              onClick={async () => {
                if ((await run(() => api.removeWorkspace(wsId))) !== undefined) {
                  setWsId(null);
                  await refreshApp();
                }
              }}
            >
              Remove
            </Button>
            <Button variant="link" onClick={() => setConfirmRemove(false)}>
              Cancel
            </Button>
          </>
        ) : (
          <Button variant="link" className="hover:text-bad" onClick={() => setConfirmRemove(true)}>
            Remove workspace
          </Button>
        )}
      </div>
    </Page>
  );
}
