import type { EffectiveConfig, OdooLaunch, Prd } from "./types";

// Example data so the UI is usable in a plain browser (`pnpm dev`) and before
// the engine is wired up. Clearly fake; never mixed with real workspace data.

export const DEMO_PRDS: Prd[] = [
  {
    id: "SP-016",
    title: "Kurs otomatis dari Bank Indonesia",
    type: "feature",
    module: "account_custom",
    step: 4,
    status: "ai",
    sub: "Reading the code",
    author: "SP",
    timeline: [
      { state: "done", text: "Request logged" },
      { state: "done", text: "Found 2 related notes in the brain" },
      { state: "run", text: "Reading account_custom on preprod" },
    ],
  },
  {
    id: "SP-011",
    title: "Diskon bertingkat per pelanggan grosir",
    type: "feature",
    module: "sale_custom",
    step: 6,
    status: "needs",
    action: "Answer 3 questions",
    sub: "Questions ready",
    author: "SP",
    timeline: [
      { state: "done", text: "Read sale.order and res.partner" },
      { state: "info", text: "3 questions waiting for you" },
    ],
  },
  {
    id: "SP-012",
    title: "Invoice dobel saat pengiriman parsial",
    type: "bug",
    module: "sale_custom",
    step: 9,
    status: "ai",
    sub: "Making the tests pass, try 2 of 5",
    author: "SP",
    timeline: [
      { state: "done", text: "Created branch feature/SP-012-invoice-dobel" },
      { state: "fail", text: "One test still failing: backorder twice" },
      { state: "run", text: "Following the backorder chain" },
    ],
  },
  {
    id: "SP-014",
    title: "Laporan stok per gudang",
    type: "feature",
    module: "stock_report",
    step: 10,
    status: "wait",
    sub: "Waiting for the database",
    author: "SP",
    timeline: [{ state: "run", text: "Waiting for the database" }],
  },
  {
    id: "SP-009",
    title: "Validasi format NPWP 16 digit",
    type: "improve",
    module: "l10n_id_custom",
    step: 14,
    status: "needs",
    action: "Test it in Odoo",
    sub: "Ready to test",
    author: "SP",
    timeline: [{ state: "info", text: "5 test cases ready" }],
  },
  {
    id: "AR-004",
    title: "Import pricelist dari Excel",
    type: "feature",
    module: "sale_custom",
    step: 20,
    status: "done",
    sub: "Merged by Andi",
    author: "AR",
    timeline: [{ state: "done", text: "PR #47 merged into preprod" }],
  },
];

export const DEMO_CONFIG: EffectiveConfig = {
  team: {
    preset: "odoo",
    language: "id",
    base_branch: "preprod",
    feature_branch: "feature/{prd_id}-{slug}",
    commit_format: "odoo",
    steps: {
      prd: { skills: ["grill-me"], max_iterations: 5, max_minutes: 30, context_tokens: 6000 },
      develop: { skills: ["tdd", "sdd"], max_iterations: 5, max_minutes: 30, context_tokens: 10000 },
      test: { skills: [], max_iterations: 5, max_minutes: 30, context_tokens: 6000 },
      commit: { skills: [], max_iterations: 5, max_minutes: 30, context_tokens: 3000 },
      brain: { skills: [], max_iterations: 5, max_minutes: 30, context_tokens: 3000 },
      full_suite: false,
    },
  },
  workspace: {
    name: "acme-odoo",
    initials: "SP",
    db: "acme_dev",
    http_port: 8069,
    launch_config: "Odoo 17 (acme)",
    github_token_ref: null,
    ai: { mode: "subscription" },
    models: { default: "claude-sonnet-5-5", prd: null, develop: null, brain: null, commit: null },
  },
};

export const DEMO_LAUNCHES: OdooLaunch[] = [
  {
    name: "Odoo 17 (acme)",
    python: "${workspaceFolder}/.venv/bin/python",
    odoo_bin: "/opt/odoo/17.0/odoo-bin",
    base_args: ["-c", "odoo17.conf", "--dev=all"],
    config_file: "odoo17.conf",
  },
];
