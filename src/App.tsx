import { useEffect, useState } from "react";
import { HashRouter, Route, Routes } from "react-router";

import { Sidebar, type Theme } from "@/components/sidebar";
import { listPrds } from "@/lib/api";
import type { Prd } from "@/lib/types";
import { BoardPage } from "@/pages/board";
import { PlaceholderPage } from "@/pages/placeholder";
import { PrdDetailPage } from "@/pages/prd-detail";
import { SettingsPage } from "@/pages/settings";

const NEXT_THEME: Record<Theme, Theme> = { system: "light", light: "dark", dark: "system" };

export default function App() {
  const [prds, setPrds] = useState<Prd[]>([]);
  const [theme, setTheme] = useState<Theme>("system");
  const workspace = "acme-odoo";

  useEffect(() => {
    void listPrds().then(setPrds);
  }, []);

  useEffect(() => {
    const root = document.documentElement;
    if (theme === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", theme);
  }, [theme]);

  return (
    <HashRouter>
      <div className="flex h-full">
        <Sidebar
          workspace={workspace}
          needsYou={prds.filter((p) => p.status === "needs").length}
          theme={theme}
          onTheme={() => setTheme(NEXT_THEME[theme])}
        />
        <main className="min-w-0 flex-1 overflow-auto">
          <Routes>
            <Route path="/" element={<BoardPage workspace={workspace} prds={prds} />} />
            <Route path="/prd/:id" element={<PrdDetailPage prds={prds} />} />
            <Route path="/brain" element={<PlaceholderPage title="Brain" about="Decisions, rules, flows, people and the graph, shared with the team." />} />
            <Route path="/skills" element={<PlaceholderPage title="Skills" about="Which Claude Code skills run at each step." />} />
            <Route path="/database" element={<PlaceholderPage title="Database" about="Who is using the shared local database, and who is waiting." />} />
            <Route path="/settings" element={<SettingsPage />} />
          </Routes>
        </main>
      </div>
    </HashRouter>
  );
}
