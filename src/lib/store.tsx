import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState, type ReactNode } from "react";

import { api, subscribe } from "./api";
import type { AppView, Prd, ServerEvent, WorkspaceView } from "./types";

interface Toast {
  id: number;
  title: string;
  body: string;
  kind: "you" | "ok" | "bad" | "info";
}

interface Store {
  app: AppView | null;
  loadError: string | null;
  wsId: string | null;
  setWsId: (id: string | null) => void;
  workspace: WorkspaceView | null;
  prds: Prd[];
  brainVersion: number;
  connected: boolean;
  toasts: Toast[];
  toast: (title: string, body?: string, kind?: Toast["kind"]) => void;
  refreshApp: () => Promise<void>;
  refreshWorkspace: () => Promise<void>;
  upsertPrd: (p: Prd) => void;
}

const Ctx = createContext<Store | null>(null);
const WS_KEY = "floww.workspace";

function savedWs(): string | null {
  try {
    return localStorage.getItem(WS_KEY);
  } catch {
    return null;
  }
}

export function StoreProvider({ children }: { children: ReactNode }) {
  const [app, setApp] = useState<AppView | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [wsId, setWsIdState] = useState<string | null>(savedWs);
  const [workspace, setWorkspace] = useState<WorkspaceView | null>(null);
  const [prds, setPrds] = useState<Prd[]>([]);
  const [brainVersion, setBrainVersion] = useState(0);
  const [connected, setConnected] = useState(true);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const wsRef = useRef(wsId);
  wsRef.current = wsId;

  const toast = useCallback((title: string, body = "", kind: Toast["kind"] = "info") => {
    const id = Date.now() + Math.random();
    setToasts((t) => [...t.slice(-2), { id, title, body, kind }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 4500);
  }, []);

  const setWsId = useCallback((id: string | null) => {
    setWsIdState(id);
    try {
      if (id) localStorage.setItem(WS_KEY, id);
      else localStorage.removeItem(WS_KEY);
    } catch {
      // Remembering the workspace is a convenience only.
    }
  }, []);

  const refreshApp = useCallback(async () => {
    try {
      const a = await api.app();
      setApp(a);
      setLoadError(null);
      const current = wsRef.current;
      if (!current || !a.workspaces.some((w) => w.id === current)) {
        setWsId(a.workspaces[0]?.id ?? null);
      }
    } catch (e) {
      setLoadError(String((e as Error).message ?? e));
    }
  }, [setWsId]);

  const refreshWorkspace = useCallback(async () => {
    const id = wsRef.current;
    if (!id) {
      setWorkspace(null);
      setPrds([]);
      return;
    }
    try {
      const [w, p] = await Promise.all([api.workspace(id), api.prds(id)]);
      if (wsRef.current !== id) return;
      setWorkspace(w);
      setPrds(p);
    } catch (e) {
      toast("Could not load the workspace", String((e as Error).message ?? e), "bad");
    }
  }, [toast]);

  const upsertPrd = useCallback((p: Prd) => {
    setPrds((list) => {
      const i = list.findIndex((x) => x.id === p.id);
      if (i === -1) return [p, ...list];
      const next = list.slice();
      next[i] = p;
      return next;
    });
  }, []);

  useEffect(() => {
    void refreshApp();
  }, [refreshApp]);

  useEffect(() => {
    void refreshWorkspace();
  }, [wsId, refreshWorkspace]);

  useEffect(() => {
    const onEvent = (e: ServerEvent) => {
      switch (e.type) {
        case "prd_changed":
          if (e.workspace === wsRef.current) upsertPrd(e.prd);
          void refreshApp();
          break;
        case "workspaces_changed":
          void refreshApp();
          break;
        case "lock_changed":
          if (e.workspace === wsRef.current) {
            void api.lock(e.workspace).then((lock) => setWorkspace((w) => (w ? { ...w, lock } : w))).catch(() => undefined);
          }
          break;
        case "brain_changed":
          if (e.workspace === wsRef.current) setBrainVersion((v) => v + 1);
          break;
        case "notify":
          toast(e.title, e.body, e.kind);
          if (document.hidden && "Notification" in window && Notification.permission === "granted") {
            new Notification(e.title, { body: e.body });
          }
          break;
      }
    };
    return subscribe(onEvent, setConnected);
  }, [refreshApp, toast, upsertPrd]);

  const value = useMemo<Store>(
    () => ({ app, loadError, wsId, setWsId, workspace, prds, brainVersion, connected, toasts, toast, refreshApp, refreshWorkspace, upsertPrd }),
    [app, loadError, wsId, setWsId, workspace, prds, brainVersion, connected, toasts, toast, refreshApp, refreshWorkspace, upsertPrd],
  );
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useStore(): Store {
  const s = useContext(Ctx);
  if (!s) throw new Error("useStore outside StoreProvider");
  return s;
}

/** Run an API call; show its error as a toast. Returns undefined on error. */
export function useAction() {
  const { toast } = useStore();
  const [busy, setBusy] = useState(false);
  const run = useCallback(
    async <T,>(fn: () => Promise<T>, done?: string): Promise<T | undefined> => {
      setBusy(true);
      try {
        const r = await fn();
        if (done) toast(done, "", "ok");
        return r;
      } catch (e) {
        toast("That did not work", String((e as Error).message ?? e), "bad");
        return undefined;
      } finally {
        setBusy(false);
      }
    },
    [toast],
  );
  return { run, busy };
}
