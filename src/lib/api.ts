import { invoke } from "@tauri-apps/api/core";

import { DEMO_CONFIG, DEMO_LAUNCHES, DEMO_PRDS } from "./demo";
import type { EffectiveConfig, OdooLaunch, Prd, StepInfo } from "./types";

/** True inside the desktop app, false in a plain browser (`pnpm dev`). */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

export async function appInfo(): Promise<{ version: string }> {
  return inTauri ? invoke("app_info") : { version: "dev" };
}

export async function pipelineSteps(): Promise<StepInfo[]> {
  return inTauri ? invoke("pipeline_steps") : [];
}

export async function loadWorkspace(repo: string): Promise<EffectiveConfig> {
  return inTauri ? invoke("load_workspace", { repo }) : structuredClone(DEMO_CONFIG);
}

export async function launchConfigs(repo: string): Promise<OdooLaunch[]> {
  return inTauri ? invoke("launch_configs", { repo }) : DEMO_LAUNCHES;
}

export async function nextPrdId(repo: string, initials: string): Promise<string> {
  return inTauri ? invoke("next_prd_id", { repo, initials }) : `${initials}-017`;
}

/** PRDs come from the pipeline engine later; example data until then. */
export async function listPrds(): Promise<Prd[]> {
  return DEMO_PRDS;
}
