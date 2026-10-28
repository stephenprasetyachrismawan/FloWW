import { Database, LayoutPanelLeft, Monitor, Moon, Network, SlidersHorizontal, Sun, WandSparkles } from "lucide-react";
import { NavLink } from "react-router";

import { cn } from "@/lib/utils";

const NAV = [
  { to: "/", label: "Board", icon: LayoutPanelLeft, end: true },
  { to: "/brain", label: "Brain", icon: Network },
  { to: "/skills", label: "Skills", icon: WandSparkles },
  { to: "/database", label: "Database", icon: Database },
  { to: "/settings", label: "Settings", icon: SlidersHorizontal },
];

export type Theme = "system" | "light" | "dark";

export function Sidebar({
  workspace,
  needsYou,
  theme,
  onTheme,
}: {
  workspace: string;
  needsYou: number;
  theme: Theme;
  onTheme: () => void;
}) {
  const ThemeIcon = theme === "light" ? Sun : theme === "dark" ? Moon : Monitor;
  return (
    <aside className="flex w-[212px] shrink-0 flex-col border-r border-line px-2.5 py-3.5">
      <div className="truncate px-2.5 py-2 font-semibold text-ink">{workspace}</div>
      <nav className="mt-4 flex flex-col gap-px">
        {NAV.map(({ to, label, icon: Icon, end }) => (
          <NavLink
            key={to}
            to={to}
            end={end}
            className={({ isActive }) =>
              cn(
                "flex items-center gap-2.5 rounded-lg px-2.5 py-[7px] font-medium text-muted hover:bg-sunken hover:text-ink",
                isActive && "bg-sunken font-semibold text-ink",
              )
            }
          >
            <Icon className="size-[17px]" strokeWidth={1.8} />
            <span>{label}</span>
            {label === "Board" && needsYou > 0 ? (
              <span className="ml-auto text-xs font-semibold text-you tabular-nums">{needsYou}</span>
            ) : null}
          </NavLink>
        ))}
      </nav>
      <div className="mt-auto flex flex-col gap-0.5 pt-2.5">
        <button
          type="button"
          onClick={onTheme}
          className="flex cursor-pointer items-center gap-2.5 rounded-lg px-2.5 py-[7px] text-[13px] text-faint hover:bg-sunken hover:text-ink"
        >
          <ThemeIcon className="size-[15px]" strokeWidth={1.8} />
          Theme: {theme}
        </button>
      </div>
    </aside>
  );
}
