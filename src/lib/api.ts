import { invoke } from "@tauri-apps/api/core";

import type {
  AppView, Detection, FolderListing, Graph, UpdateCheck, LockState, Note, Pending, Prd, PrdType, ScanReport, ServerEvent, Skill, WorkspaceView,
} from "./types";

/** True inside the desktop app. */
export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

interface Conn {
  base: string;
  token: string;
}

let conn: Conn | null = null;
const TOKEN_KEY = "floww.token";

function readToken(): string {
  const url = new URL(window.location.href);
  const fromUrl = url.searchParams.get("token");
  if (fromUrl) {
    try {
      sessionStorage.setItem(TOKEN_KEY, fromUrl);
    } catch {
      // The token still works for this page load.
    }
    url.searchParams.delete("token");
    window.history.replaceState(null, "", url.pathname + url.search + url.hash);
    return fromUrl;
  }
  try {
    return sessionStorage.getItem(TOKEN_KEY) ?? "";
  } catch {
    return "";
  }
}

/** Find the local server: the desktop app asks Tauri, the browser uses its own origin. */
export async function connect(): Promise<Conn> {
  if (conn) return conn;
  if (inTauri) {
    const info = await invoke<{ url: string; token: string; error: string | null }>("server_info");
    if (info.error) throw new Error(info.error);
    conn = { base: info.url, token: info.token };
  } else {
    conn = { base: "", token: readToken() };
  }
  return conn;
}

export class ApiError extends Error {
  status: number;
  constructor(message: string, status: number) {
    super(message);
    this.status = status;
  }
}

async function call<T>(method: string, path: string, body?: unknown): Promise<T> {
  const c = await connect();
  const res = await fetch(`${c.base}/api${path}`, {
    method,
    headers: { Authorization: `Bearer ${c.token}`, ...(body !== undefined ? { "Content-Type": "application/json" } : {}) },
    body: body !== undefined ? JSON.stringify(body) : undefined,
  });
  const text = await res.text();
  const data = text ? JSON.parse(text) : {};
  if (!res.ok) throw new ApiError(data.error ?? `Request failed (${res.status})`, res.status);
  return data as T;
}

const enc = encodeURIComponent;

export const api = {
  app: () => call<AppView>("GET", "/app"),
  setAi: (b: { mode: "subscription" | "router"; url?: string; token?: string; model?: string | null }) => call("POST", "/ai", b),
  testAi: (model?: string | null) => call<{ message: string }>("POST", "/ai/test", { model }),
  models: () => call<string[]>("GET", "/models"),
  detect: (path: string) => call<Detection>("POST", "/detect", { path }),
  updateCheck: () => call<UpdateCheck>("GET", "/update"),
  updateApply: () => call<{ latest: string }>("POST", "/update/apply"),
  branchPreview: (template: string, initials?: string) =>
    call<{ name: string }>("GET", `/branch-preview?template=${enc(template)}${initials ? `&initials=${enc(initials)}` : ""}`),
  folders: (path?: string) => call<FolderListing>("GET", `/folders${path ? `?path=${enc(path)}` : ""}`),
  addWorkspace: (b: {
    path: string; name: string; initials: string; db: string; base_branch: string;
    launch_config: string | null; http_port: number; language?: string; branch_template?: string;
  }) => call<{ id: string }>("POST", "/workspaces", b),
  workspace: (ws: string) => call<WorkspaceView>("GET", `/workspaces/${ws}`),
  removeWorkspace: (ws: string) => call("DELETE", `/workspaces/${ws}`),
  saveSettings: (ws: string, b: Record<string, unknown>) => call<WorkspaceView>("PUT", `/workspaces/${ws}/settings`, b),
  setStepSkills: (ws: string, step: string, skills: string[]) => call("PUT", `/workspaces/${ws}/skills/${step}`, { skills }),
  prds: (ws: string) => call<Prd[]>("GET", `/workspaces/${ws}/prds`),
  createPrd: (ws: string, b: { title: string; description: string; kind: PrdType; reporter: string }) =>
    call<{ id: string }>("POST", `/workspaces/${ws}/prds`, b),
  prdHtml: (ws: string, id: string) => call<{ html: string; markdown: string }>("GET", `/workspaces/${ws}/prds/${id}/html`),
  diff: (ws: string, id: string) => call<{ files: string[]; stat?: string; diff: string }>("GET", `/workspaces/${ws}/prds/${id}/diff`),
  odooLog: (ws: string, id: string) => call<{ log: string }>("GET", `/workspaces/${ws}/prds/${id}/odoo-log`),
  act: (ws: string, id: string, action: string, body?: Record<string, unknown>) =>
    call<Prd & { _state?: string; _paused?: string | null }>("POST", `/workspaces/${ws}/prds/${id}/do/${action}`, body ?? {}),
  lock: (ws: string) => call<LockState>("GET", `/workspaces/${ws}/lock`),
  brain: (ws: string, folder: string) => call<Note[]>("GET", `/workspaces/${ws}/brain?folder=${enc(folder)}`),
  brainNote: (ws: string, path: string) => call<{ note: Note; text: string }>("GET", `/workspaces/${ws}/brain/note?path=${enc(path)}`),
  confirmNote: (ws: string, path: string) => call("POST", `/workspaces/${ws}/brain/confirm`, { path }),
  brainSync: (ws: string) => call("POST", `/workspaces/${ws}/brain/sync`),
  refreshFacts: (ws: string) => call<{ modules: number }>("POST", `/workspaces/${ws}/brain/refresh`),
  pending: (ws: string) => call<Pending[]>("GET", `/workspaces/${ws}/pending`),
  resolvePending: (ws: string, path: string, accept: boolean, text?: string) => call("POST", `/workspaces/${ws}/pending`, { path, accept, text }),
  graph: (ws: string) => call<Graph>("GET", `/workspaces/${ws}/graph`),
  skills: (ws?: string) => call<Skill[]>("GET", `/skills${ws ? `?ws=${ws}` : ""}`),
  installSkill: (source: string, force = false) => call<{ name: string; installed: boolean; report: ScanReport }>("POST", "/skills/install", { source, force }),
};

/** Live updates. Reconnects by itself. */
export function subscribe(onEvent: (e: ServerEvent) => void, onStatus?: (connected: boolean) => void): () => void {
  let es: EventSource | null = null;
  let stopped = false;
  let retry: ReturnType<typeof setTimeout> | undefined;
  const open = async () => {
    const c = await connect();
    if (stopped) return;
    es = new EventSource(`${c.base}/api/events?token=${enc(c.token)}`);
    es.onopen = () => onStatus?.(true);
    es.onmessage = (m) => {
      try {
        onEvent(JSON.parse(m.data) as ServerEvent);
      } catch {
        // Ignore malformed events.
      }
    };
    es.onerror = () => {
      onStatus?.(false);
      es?.close();
      if (!stopped) retry = setTimeout(open, 2000);
    };
  };
  void open();
  return () => {
    stopped = true;
    clearTimeout(retry);
    es?.close();
  };
}

/** Desktop: close FloWW and let the updater window install the new version. */
export async function startDesktopUpdate(): Promise<void> {
  await invoke("start_update");
}

/** The system folder picker in the desktop app. Returns null when cancelled. */
export async function pickFolderNative(title: string): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ directory: true, multiple: false, title });
  return typeof picked === "string" ? picked : null;
}

/** Open a link in the real browser (desktop) or a new tab (web). */
export async function openExternal(url: string) {
  if (inTauri) {
    const { openUrl } = await import("@tauri-apps/plugin-opener");
    await openUrl(url);
  } else {
    window.open(url, "_blank", "noopener");
  }
}
