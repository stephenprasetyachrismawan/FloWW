// Clicks through FloWW in a real browser: first run → add workspace → request →
// grill → approve → build → test in Odoo → commit → push → done.
// Uses a real git repo + remote and fake `claude` / `odoo-bin` scripts.
//
// Run: pnpm build && cargo build -p floww-server && node e2e/ui-flow.mjs
import { execFileSync, spawn } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const { chromium } = require("playwright");

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), "..");
const tmp = fs.mkdtempSync(path.join(os.tmpdir(), "floww-ui-"));
const shots = process.env.SHOTS ?? path.join(tmp, "shots");
fs.mkdirSync(shots, { recursive: true });
const bin = path.join(tmp, "bin");
const repo = path.join(tmp, "acme-odoo");
const remote = path.join(tmp, "remote.git");
fs.mkdirSync(bin);

const FAKE_CLAUDE = `#!/usr/bin/env python3
import json, os, subprocess, sys, time
args = sys.argv[1:]
prompt = args[args.index("-p") + 1]
def emit(o): print(json.dumps(o), flush=True)
emit({"type": "system", "subtype": "init", "session_id": "s1", "model": "fake"})
emit({"type": "assistant", "message": {"content": [{"type": "tool_use", "name": "Read", "input": {"file_path": "addons/sale_custom/models/sale.py"}}]}})
time.sleep(0.4)
if "Find the code this request is about" in prompt:
    out = 'Qty to invoice counts cancelled moves.\\n\`\`\`json\\n{"summary": "Qty to invoice counts cancelled moves too.", "modules": ["sale_custom"], "files": ["addons/sale_custom/models/sale.py"], "similar_prd": null}\\n\`\`\`'
elif "Interview the person" in prompt:
    out = '\`\`\`json\\n{"done": false, "question": {"topic": "Scope", "text": "Should cancelled deliveries count toward the invoiced quantity?", "explain": null, "options": [{"label": "No, ignore cancelled moves", "why": "Nothing was delivered", "recommended": true}, {"label": "Yes, keep counting them", "why": "Matches today", "recommended": false}]}}\\n\`\`\`'
elif prompt.startswith("The person chose") or prompt.startswith("The person wrote"):
    out = '\`\`\`json\\n{"done": true}\\n\`\`\`'
elif "Write the PRD now" in prompt:
    out = '# Invoice dobel saat pengiriman parsial\\n\\n## Masalah\\nCancelled moves are counted, so Create Invoice bills twice.\\n\\n## Tujuan\\nInvoiced quantity is never invoiced again.\\n\\n| Bagian | Perubahan |\\n|---|---|\\n| sale.order.line | Skip cancelled moves |\\n\\n\`\`\`json\\n{"criteria": [{"given": "a delivery cancelled then redelivered", "when": "Create Invoice is pressed", "then": "only one invoice is made"}], "tests": [{"kind": "Normal", "title": "Full delivery invoices once", "steps": "Deliver 10, Create Invoice", "data": "SO/2026/0412", "expected": "One invoice, qty 10"}, {"kind": "Edge", "title": "Cancel then redeliver", "steps": "Deliver 4, cancel, redeliver 4, invoice", "data": "SO/2026/0388", "expected": "One invoice, qty 4"}], "rules": [{"id": "no-double-invoice", "title": "Invoiced quantity is never invoiced again", "why": "7 double invoices in October", "enforced_in": "sale.order.line._compute_qty_to_invoice"}], "modules": ["sale_custom"]}\\n\`\`\`'
elif "acceptance criterion" in prompt:
    p = os.path.join("addons", "sale_custom", "models", "sale.py")
    with open(p, "a") as f: f.write("\\n# skip cancelled moves\\n")
    subprocess.run(["git", "add", "-A"], check=True)
    subprocess.run(["git", "commit", "-q", "-m", "wip: criterion"], check=True)
    out = "Tests pass."
elif "Write the commit message" in prompt:
    out = '\`\`\`json\\n{"message": "[FIX] sale_custom: ignore cancelled moves when invoicing\\\\n\\\\nPRD: SP-001", "pr_title": "[FIX] sale_custom: ignore cancelled moves", "pr_body": "PRD: SP-001"}\\n\`\`\`'
else:
    out = "OK"
emit({"type": "result", "subtype": "success", "is_error": False, "session_id": "s1", "result": out, "total_cost_usd": 0.02, "usage": {"input_tokens": 1200, "output_tokens": 200}})
`;
const FAKE_ODOO = `#!/usr/bin/env python3
import sys, time
if "--stop-after-init" in sys.argv:
    print("done"); sys.exit(0)
time.sleep(600)
`;

const write = (p, t, mode) => {
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, t);
  if (mode) fs.chmodSync(p, mode);
};
const git = (cwd, ...a) => execFileSync("git", a, { cwd, stdio: "pipe" });

write(path.join(bin, "claude"), FAKE_CLAUDE, 0o755);
write(path.join(bin, "odoo-bin"), FAKE_ODOO, 0o755);
git(tmp, "init", "-q", "--bare", "-b", "main", remote);
write(path.join(repo, "addons/sale_custom/__manifest__.py"), "{'name': 'Sale custom', 'version': '17.0.1.0.0', 'depends': ['sale']}");
write(path.join(repo, "addons/sale_custom/models/sale.py"), "from odoo import fields, models\n\nclass SaleOrderLine(models.Model):\n    _inherit = 'sale.order.line'\n\n    def _compute_qty_to_invoice(self):\n        pass\n");
write(path.join(repo, ".vscode/launch.json"), JSON.stringify({ configurations: [{ name: "Odoo 17 (acme)", program: path.join(bin, "odoo-bin"), args: ["-c", "odoo.conf"] }] }));
git(repo, "init", "-q", "-b", "main");
git(repo, "-c", "user.name=Stephen Prasetya", "-c", "user.email=s@example.com", "add", "-A");
git(repo, "-c", "user.name=Stephen Prasetya", "-c", "user.email=s@example.com", "commit", "-q", "-m", "init");
git(repo, "config", "user.name", "Stephen Prasetya");
git(repo, "config", "user.email", "s@example.com");
git(repo, "remote", "add", "origin", remote);
git(repo, "push", "-q", "-u", "origin", "main");

const port = 7300 + Math.floor(Math.random() * 500);
const token = "e2e-token-" + Date.now();
const server = spawn(path.join(root, "target/debug/floww-web"), ["--port", String(port), "--token", token], {
  env: { ...process.env, FLOWW_HOME: path.join(tmp, "home"), FLOWW_CLAUDE_BIN: path.join(bin, "claude") },
  stdio: ["ignore", "pipe", "inherit"],
});
await new Promise((resolve, reject) => {
  server.stdout.on("data", (d) => String(d).includes("FloWW is running") && resolve());
  server.on("exit", (c) => reject(new Error(`floww-web exited ${c}`)));
  setTimeout(() => reject(new Error("floww-web did not start")), 15000);
});

const browser = await chromium.launch();
const page = await browser.newPage({ viewport: { width: 1280, height: 860 } });
const errors = [];
page.on("pageerror", (e) => errors.push(e.message));
let n = 0;
const shot = async (name) => page.screenshot({ path: path.join(shots, `${String(++n).padStart(2, "0")}-${name}.png`) });
const T = { timeout: 20000 };

try {
  await page.goto(`http://127.0.0.1:${port}/?token=${token}`);
  // First run
  await page.getByText("How should FloWW talk to Claude?").waitFor(T);
  await shot("first-run");
  await page.getByRole("button", { name: "Continue" }).click();
  await page.getByText("Which model should it use?").waitFor(T);
  await page.getByRole("button", { name: /claude-sonnet-5-5/ }).click();
  await page.getByRole("button", { name: "Done" }).click();
  // Add workspace
  await page.getByText("Add your first project").waitFor(T);
  await page.getByRole("button", { name: "Add workspace" }).click();
  // A typo gets a clear error with the close match.
  await page.getByLabel("Repository folder").fill(path.join(tmp, "Acme-Odoo") + "/");
  await page.getByRole("button", { name: "Look at this folder" }).click();
  await page.getByText("Did you mean").waitFor(T);
  await shot("workspace-typo");
  // Pick it with the folder picker instead.
  await page.getByLabel("Repository folder").fill(tmp);
  await page.getByRole("button", { name: "Choose folder…" }).click();
  const row = page.getByRole("listitem").filter({ hasText: "acme-odoo" });
  await row.getByText("git repo").waitFor(T);
  await shot("workspace-picker");
  await row.getByRole("button", { name: "Choose" }).click();
  await page.getByText("Look right?").waitFor(T);
  await shot("workspace-review");
  await page.getByRole("button", { name: "Create workspace" }).click();
  await page.getByText("No requests yet").waitFor(T);
  // New request
  await page.getByRole("button", { name: "New request" }).click();
  await page.getByLabel("What is it about?").fill("Invoice dobel saat pengiriman parsial");
  await page.getByLabel("Details").fill("Finance: SO/2026/0451 delivered partially, then invoiced twice.");
  await page.getByRole("button", { name: "Create request" }).click();
  await page.getByText("Should cancelled deliveries count").waitFor(T);
  await shot("grill");
  await page.getByRole("button", { name: /No, ignore cancelled moves/ }).click();
  await page.getByText("That is everything the AI needed to ask.").waitFor(T);
  await page.getByRole("button", { name: "Write the PRD" }).click();
  await page.getByText("The PRD is ready.").waitFor(T);
  await shot("approve");
  await page.getByRole("button", { name: "Approve and start building" }).click();
  await page.getByText("Test it yourself in Odoo.").waitFor({ timeout: 40000 });
  await page.getByRole("button", { name: "Start Odoo" }).click();
  await page.getByText("Odoo is running").waitFor(T);
  await shot("human-test");
  // Mark every case as working, one at a time.
  const works = page.getByRole("button", { name: "Works" });
  const total = await works.count();
  for (let i = 0; i < total; i++) {
    await works.nth(i).click();
    await page.waitForFunction((idx) => document.querySelectorAll("button")[0] && [...document.querySelectorAll("button")].filter((b) => b.textContent === "Works")[idx]?.className.includes("text-ok"), i, T);
  }
  await page.getByRole("button", { name: "Everything works, ship it" }).click(T);
  await page.getByText("Check the commit message.").waitFor(T);
  await shot("ship");
  await page.getByRole("button", { name: "Commit", exact: true }).click();
  await page.getByRole("button", { name: "Push branch" }).click();
  await page.getByText("Pushed. Open a pull request?").waitFor(T);
  await page.getByRole("button", { name: "Skip" }).click();
  await page.getByText("The brain has the notes").waitFor(T);
  await shot("done");
  // Brain shows the rule that came from the PRD.
  await page.getByRole("link", { name: "Brain" }).click();
  await page.getByRole("button", { name: "Rules" }).click();
  await page.getByText("Invoiced quantity is never invoiced again").waitFor(T);
  await page.getByRole("button", { name: "Confirm" }).click();
  await page.getByText("Confirmed", { exact: true }).waitFor(T);
  await shot("brain-rules");
  await page.getByRole("button", { name: "Graph" }).click();
  await page.waitForTimeout(500);
  await shot("graph");

  const branches = execFileSync("git", ["branch", "--list", "feature/*"], { cwd: remote }).toString();
  if (!branches.includes("feature/SP-001")) throw new Error("branch was not pushed: " + branches);
  if (errors.length) throw new Error("page errors: " + errors.join("; "));
  console.log(`UI FLOW OK · screenshots in ${shots}`);
} catch (e) {
  await shot("failure").catch(() => undefined);
  console.error("UI FLOW FAILED:", e.message, `\nscreenshots in ${shots}`);
  process.exitCode = 1;
} finally {
  await browser.close();
  server.kill();
}
