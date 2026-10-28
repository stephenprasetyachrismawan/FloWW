// Mirrors the serde output of `floww-core`. Keep in sync with crates/floww-core/src.

export type Phase = "request" | "planning" | "building" | "testing" | "ship" | "done";
export type Actor = "ai" | "human" | "app";

export interface StepInfo {
  number: number;
  name: string;
  phase: Phase;
  actor: Actor;
  needs_db_lock: boolean;
}

export interface StepSettings {
  skills: string[];
  max_iterations: number;
  max_minutes: number;
  context_tokens: number;
}

export interface TeamConfig {
  preset: "odoo" | "custom";
  language: string;
  base_branch: string;
  feature_branch: string;
  commit_format: "odoo" | "conventional";
  steps: {
    prd: StepSettings;
    develop: StepSettings;
    test: StepSettings;
    commit: StepSettings;
    brain: StepSettings;
    full_suite: boolean;
  };
}

export type AiAuth = { mode: "subscription" } | { mode: "router"; url: string; token_ref: string };

export interface WorkspaceConfig {
  name: string;
  initials: string;
  db: string;
  http_port: number;
  launch_config: string | null;
  github_token_ref: string | null;
  ai: AiAuth | null;
  models: {
    default: string | null;
    prd: string | null;
    develop: string | null;
    brain: string | null;
    commit: string | null;
  };
}

export interface EffectiveConfig {
  team: TeamConfig;
  workspace: WorkspaceConfig;
}

export interface OdooLaunch {
  name: string;
  python: string | null;
  odoo_bin: string;
  base_args: string[];
  config_file: string | null;
}

// UI-side models (the engine that fills these comes in a later change).

export type PrdType = "bug" | "feature" | "improve";
export type PrdStatus = "ai" | "needs" | "wait" | "review" | "done";

export interface Prd {
  id: string;
  title: string;
  type: PrdType;
  module: string;
  step: number;
  status: PrdStatus;
  /** What the person should do, when status is "needs". */
  action?: string;
  /** One-line status shown on the card. */
  sub: string;
  author: string;
  timeline: { state: "done" | "run" | "fail" | "info"; text: string }[];
}
