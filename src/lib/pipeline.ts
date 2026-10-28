import type { Prd, Step } from "./types";

export type Phase = "request" | "planning" | "building" | "testing" | "ship" | "done";

/** Same grouping as `Step::phase` in crates/floww-core/src/pipeline.rs. */
const PHASE_OF: Record<Step, Phase> = {
  new_request: "request",
  check_brain: "request",
  read_database: "request",
  read_codebase: "request",
  update_brain: "request",
  grill_prd: "planning",
  approve_prd: "planning",
  create_branch: "building",
  develop: "building",
  full_suite: "building",
  log_changes: "building",
  start_odoo: "testing",
  upgrade_modules: "testing",
  human_test: "testing",
  feedback: "testing",
  commit_message: "ship",
  commit: "ship",
  push: "ship",
  pull_request: "ship",
  done: "done",
};

export const PHASES: { key: Phase; label: string }[] = [
  { key: "request", label: "Request" },
  { key: "planning", label: "Planning" },
  { key: "building", label: "Building" },
  { key: "testing", label: "Testing" },
  { key: "ship", label: "Ship" },
  { key: "done", label: "Done" },
];

export const phaseOf = (step: Step): Phase => PHASE_OF[step];

export const TYPE_LABEL: Record<Prd["type"], string> = { bug: "Bug", improve: "Improvement", feature: "Feature" };

/** "Your turn" means a person has to act. */
export const needsYou = (p: Prd) => p.status === "needs" || p.status === "failed" || p.status === "paused";
