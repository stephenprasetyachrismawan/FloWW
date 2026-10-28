import { useEffect, useState } from "react";
import { HashRouter, Route, Routes } from "react-router";

import { Sidebar, type Theme } from "@/components/sidebar";
import { Toasts } from "@/components/toasts";
import { Button } from "@/components/ui/button";
import { inTauri } from "@/lib/api";
import { StoreProvider, useStore } from "@/lib/store";
import { AddWorkspace } from "@/pages/add-workspace";
import { BoardPage } from "@/pages/board";
import { BrainPage } from "@/pages/brain";
import { DatabasePage } from "@/pages/database";
import { FirstRun } from "@/pages/first-run";
import { NewRequest } from "@/pages/new-request";
import { PrdDetailPage } from "@/pages/prd-detail";
import { SettingsPage } from "@/pages/settings";
import { SkillsPage } from "@/pages/skills";

const NEXT_THEME: Record<Theme, Theme> = { system: "light", light: "dark", dark: "system" };
const THEME_KEY = "floww.theme";

function readTheme(): Theme {
  try {
    const t = localStorage.getItem(THEME_KEY);
    return t === "light" || t === "dark" ? t : "system";
  } catch {
    return "system";
  }
}

function Shell() {
  const { app, loadError, refreshApp } = useStore();
  const [theme, setTheme] = useState<Theme>(readTheme);
  const [modal, setModal] = useState<"workspace" | "request" | null>(null);

  useEffect(() => {
    const root = document.documentElement;
    if (theme === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", theme);
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch {
      // Theme still applies for this session.
    }
  }, [theme]);

  useEffect(() => {
    if ("Notification" in window && Notification.permission === "default") void Notification.requestPermission().catch(() => undefined);
  }, []);

  if (loadError) {
    return (
      <div className="mx-auto flex max-w-[560px] flex-col gap-4 px-6 py-16">
        <h1 className="font-serif text-[28px] font-semibold">FloWW could not connect</h1>
        <p className="text-muted">{loadError}</p>
        {!inTauri ? <p className="text-[13.5px] text-faint">Open the full link that floww-web printed in the terminal. It contains the access token for this run.</p> : null}
        <div>
          <Button onClick={() => void refreshApp()}>Try again</Button>
        </div>
      </div>
    );
  }
  if (!app) return <div className="p-10 text-faint">Starting FloWW…</div>;
  if (!app.ai_ready) return <FirstRun />;

  return (
    <div className="flex h-full max-md:flex-col">
      <Sidebar theme={theme} onTheme={() => setTheme(NEXT_THEME[theme])} onAddWorkspace={() => setModal("workspace")} />
      <main className="min-w-0 flex-1 overflow-auto">
        <Routes>
          <Route path="/" element={<BoardPage onNewRequest={() => setModal("request")} onAddWorkspace={() => setModal("workspace")} />} />
          <Route path="/prd/:id" element={<PrdDetailPage />} />
          <Route path="/brain" element={<BrainPage />} />
          <Route path="/skills" element={<SkillsPage />} />
          <Route path="/database" element={<DatabasePage />} />
          <Route path="/settings" element={<SettingsPage />} />
        </Routes>
      </main>
      {modal === "workspace" ? <AddWorkspace onClose={() => setModal(null)} /> : null}
      {modal === "request" ? <NewRequest onClose={() => setModal(null)} /> : null}
    </div>
  );
}

export default function App() {
  return (
    <HashRouter>
      <StoreProvider>
        <Shell />
        <Toasts />
      </StoreProvider>
    </HashRouter>
  );
}
