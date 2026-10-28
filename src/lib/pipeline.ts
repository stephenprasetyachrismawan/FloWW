import type { Phase } from "./types";

/** Same ranges as `Step::phase` in crates/floww-core/src/pipeline.rs. */
export const PHASES: { key: Phase; label: string; from: number; to: number }[] = [
  { key: "request", label: "Request", from: 1, to: 5 },
  { key: "planning", label: "Planning", from: 6, to: 7 },
  { key: "building", label: "Building", from: 8, to: 11 },
  { key: "testing", label: "Testing", from: 12, to: 15 },
  { key: "ship", label: "Ship", from: 16, to: 19 },
  { key: "done", label: "Done", from: 20, to: 20 },
];

export function phaseOf(step: number): Phase {
  return PHASES.find((p) => step >= p.from && step <= p.to)?.key ?? "done";
}
