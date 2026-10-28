import { Database, LayoutPanelLeft, Monitor, Moon, Network, Plus, SlidersHorizontal, Sun, WandSparkles } from "lucide-react";
import { NavLink, useNavigate } from "react-router";

import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { UpdateControl } from "@/components/update";
import { needsYou } from "@/lib/pipeline";
import { useStore } from "@/lib/store";
import { cn } from "@/lib/utils";

const NAV = [
  { to: "/", label: "Board", icon: LayoutPanelLeft, end: true },
  { to: "/brain", label: "Brain", icon: Network },
  { to: "/skills", label: "Skills", icon: WandSparkles },
  { to: "/database", label: "Database", icon: Database },
  { to: "/settings", label: "Settings", icon: SlidersHorizontal },
];

export type Theme = "system" | "light" | "dark";
const ADD = "__add";

export function Sidebar({ theme, onTheme, onAddWorkspace }: { theme: Theme; onTheme: () => void; onAddWorkspace: () => void }) {
  const { app, wsId, setWsId, prds, connected } = useStore();
  const navigate = useNavigate();
  const ThemeIcon = theme === "light" ? Sun : theme === "dark" ? Moon : Monitor;
  const yourTurn = prds.filter(needsYou).length;
  return (
    <aside className="flex w-[212px] shrink-0 flex-col border-r border-line px-2.5 py-3.5">
      <Select
        value={wsId ?? undefined}
        onValueChange={(v) => {
          if (v === ADD) onAddWorkspace();
          else {
            setWsId(v);
            navigate("/");
          }
        }}
      >
        <SelectTrigger aria-label="Workspace" className="border-transparent bg-transparent font-semibold hover:border-transparent hover:bg-sunken">
          <SelectValue placeholder="No workspace" />
        </SelectTrigger>
        <SelectContent>
          {app?.workspaces.map((w) => (
            <SelectItem key={w.id} value={w.id} description={w.path}>
              {w.name}
              {w.needs_you ? ` · ${w.needs_you}` : ""}
            </SelectItem>
          ))}
          <SelectItem value={ADD}>
            <span className="inline-flex items-center gap-1.5 text-accent">
              <Plus className="size-3.5" />
              Add workspace…
            </span>
          </SelectItem>
        </SelectContent>
      </Select>
      <nav className="mt-4 flex flex-col gap-px">
        {NAV.map(({ to, label, icon: Icon, end }) => (
          <NavLink
            key={to}
            to={to}
            end={end}
            className={({ isActive }) =>
              cn("flex items-center gap-2.5 rounded-lg px-2.5 py-[7px] font-medium text-muted hover:bg-sunken hover:text-ink", isActive && "bg-sunken font-semibold text-ink")
            }
          >
            <Icon className="size-[17px]" strokeWidth={1.8} />
            <span>{label}</span>
            {label === "Board" && yourTurn > 0 ? <span className="ml-auto text-xs font-semibold text-you tabular-nums">{yourTurn}</span> : null}
          </NavLink>
        ))}
      </nav>
      <div className="mt-auto flex flex-col gap-0.5 pt-2.5 text-[13px]">
        {!connected ? (
          <span className="flex items-center gap-2 px-2.5 py-1.5 text-bad">
            <i className="size-[7px] rounded-full bg-bad" />
            Reconnecting…
          </span>
        ) : null}
        <button type="button" onClick={onTheme} className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-[7px] text-faint hover:bg-sunken hover:text-ink">
          <ThemeIcon className="size-[15px]" strokeWidth={1.8} />
          Theme: {theme}
        </button>
        {app ? <UpdateControl version={app.version} /> : null}
      </div>
    </aside>
  );
}
