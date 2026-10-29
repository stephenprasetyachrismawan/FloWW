// Mirrors serde output of floww-core and floww-engine. Keep in sync with the Rust structs.

export type Step =
  | "new_request" | "check_brain" | "read_database" | "read_codebase" | "update_brain"
  | "grill_prd" | "approve_prd" | "create_branch" | "develop" | "full_suite" | "log_changes"
  | "start_odoo" | "upgrade_modules" | "human_test" | "feedback"
  | "commit_message" | "commit" | "push" | "pull_request" | "done";

export type Status = "ai" | "needs" | "wait" | "review" | "done" | "failed" | "paused";
export type PrdType = "bug" | "improve" | "feature";

export interface Entry {
  at: string;
  state: "done" | "run" | "fail" | "info";
  text: string;
  cmd?: string;
}

export interface GrillOption {
  label: string;
  why: string;
  recommended: boolean;
}
export interface GrillQuestion {
  topic: string;
  text: string;
  explain?: string;
  options: GrillOption[];
}
export interface Grill {
  session_id: string | null;
  current: GrillQuestion | null;
  answers: { question: GrillQuestion; answer: string; own: boolean }[];
  state: "asking" | "finished" | "writing" | "ready";
}

export interface TestCase {
  kind: string;
  title: string;
  steps: string;
  data: string;
  expected: string;
  result: "pass" | "fail" | null;
  note: string;
}

export interface Ship {
  stage: number;
  message: string;
  commit: string | null;
  pr_url: string | null;
  pr_number: number | null;
  pr_title: string;
  pr_body: string;
  skipped_pr: boolean;
}

export interface Prd {
  id: string;
  title: string;
  description: string;
  type: PrdType;
  reporter: string;
  author: string;
  created_at: string;
  step: Step;
  status: Status;
  action: string | null;
  sub: string;
  module: string | null;
  modules: string[];
  timeline: Entry[];
  context: { summary: string; modules: string[]; files: string[]; brain_notes: string[]; db_facts: string[]; similar: string | null } | null;
  grill: Grill | null;
  prd_markdown: string | null;
  criteria: { given: string; when: string; then: string; done: boolean }[];
  rules: { id: string; title: string; why: string; enforced_in: string }[];
  branch: string | null;
  worktree: string | null;
  base_commit: string | null;
  develop_session: string | null;
  chat: ChatMsg[];
  odoo_port: number | null;
  tests: TestCase[];
  odoo: "off" | "starting" | "on";
  feedback_rounds: number;
  feedback: string[];
  ship: Ship | null;
  cost_usd: number;
  tokens_in: number;
  tokens_out: number;
}

export type AiAuth = { mode: "subscription" } | { mode: "router"; url: string; token_ref: string };

export interface AppView {
  ai_ready: boolean;
  ai: AiAuth | null;
  default_model: string | null;
  workspaces: { id: string; name: string; path: string; needs_you: number }[];
  version: string;
}

export interface StepSettings {
  skills: string[];
  max_minutes: number;
  context_tokens: number;
}

export interface EffectiveConfig {
  team: {
    preset: string;
    language: string;
    base_branch: string;
    feature_branch: string;
    commit_format: "odoo" | "conventional";
    steps: { prd: StepSettings; develop: StepSettings; test: StepSettings; commit: StepSettings; brain: StepSettings; full_suite: boolean };
  };
  workspace: {
    name: string;
    initials: string;
    db: string;
    http_port: number;
    share_database: boolean;
    launch_config: string | null;
    github_token_ref: string | null;
    ai: AiAuth | null;
    models: { default: string | null; prd: string | null; develop: string | null; brain: string | null; commit: string | null; effort: string | null; develop_effort: string | null };
  };
}

export interface OdooLaunch {
  name: string;
  python: string | null;
  odoo_bin: string;
  base_args: string[];
  config_file: string | null;
}

export interface LockState {
  holder: string | null;
  doing: string;
  queue: string[];
}

export interface WorkspaceView {
  id: string;
  name: string;
  path: string;
  config: EffectiveConfig;
  lock: LockState;
  launches: OdooLaunch[];
  brain_ready: boolean;
}

export interface Detection {
  repo: string;
  name: string;
  is_git: boolean;
  preset: string;
  module_count: number;
  launches: OdooLaunch[];
  databases: string[];
  branches: string[];
  default_branch: string | null;
  has_preprod: boolean;
  has_remote: boolean;
  initials: string;
  claude_installed: boolean;
  graphify_installed: boolean;
  checks: { label: string; ok: boolean; detail: string }[];
  choices: { key: string; question: string; options: { value: string; why: string; recommended: boolean }[] }[];
  suggested: { base_branch: string; db: string; launch_config: string | null; http_port: number };
}

export interface Note {
  path: string;
  title: string;
  kind: string;
  trust: string;
  meta: Record<string, string>;
  excerpt: string;
}

export interface Pending {
  path: string;
  kind: string | null;
  who: string | null;
  source: string | null;
  text: string;
}

export interface Skill {
  name: string;
  description: string;
  location: string;
  path: string;
}

export interface ScanReport {
  scanner: string;
  findings: { severity: "low" | "medium" | "high"; file: string; message: string }[];
}

export interface Graph {
  nodes: { id: string; label: string; kind: "module" | "model" }[];
  edges: [string, string][];
}

export type ServerEvent =
  | { type: "prd_changed"; workspace: string; prd: Prd }
  | { type: "workspaces_changed" }
  | { type: "prd_deleted"; workspace: string; id: string }
  | { type: "lock_changed"; workspace: string }
  | { type: "brain_changed"; workspace: string }
  | { type: "notify"; title: string; body: string; kind: "you" | "ok" | "bad" | "info" }
  | ({ type: "update" } & UpdateProgress);

/** Mirrors floww_engine::update::Progress. */
export interface UpdateProgress {
  stage: "check" | "download" | "verify" | "install" | "cleanup" | "restart" | "failed";
  percent: number | null;
  message: string;
}

/** Mirrors floww_engine::update::Check. */
export interface UpdateCheck {
  current: string;
  latest: string;
  newer: boolean;
  kind: "app_image" | "deb" | "web" | "source";
  release_url: string;
  notes: string;
  asset: { name: string; url: string; size: number } | null;
  old_files: string[];
  can_apply: boolean;
  reason: string | null;
}

/** Mirrors floww_engine::folders::FolderEntry. */
export interface FolderEntry {
  name: string;
  path: string;
  is_git: boolean;
}

/** Mirrors floww_engine::folders::FolderListing. */
export interface FolderListing {
  path: string;
  parent: string | null;
  home: string;
  is_git: boolean;
  entries: FolderEntry[];
  truncated: boolean;
}

/** Mirrors floww_engine::model::ChatMsg. */
export interface ChatMsg {
  at: string;
  role: "floww" | "ai" | "tool" | "you";
  text: string;
  tool?: string;
  detail?: string;
}
