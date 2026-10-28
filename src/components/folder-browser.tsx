import { ChevronRight, CornerLeftUp, Folder, House, LoaderCircle } from "lucide-react";
import { useEffect, useState } from "react";

import { Button } from "@/components/ui/button";
import { api } from "@/lib/api";
import type { FolderListing } from "@/lib/types";
import { cn } from "@/lib/utils";

/**
 * Folder picker for the web UI, where the browser cannot hand out real paths.
 * The server lists folders on the computer that runs FloWW.
 */
export function FolderBrowser({ start, onChange, onPick }: { start?: string; onChange: (path: string) => void; onPick: (path: string) => void }) {
  const [listing, setListing] = useState<FolderListing | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);

  async function go(path?: string) {
    setLoading(true);
    setError(null);
    try {
      const l = await api.folders(path);
      setListing(l);
      onChange(l.path);
    } catch (e) {
      setError((e as Error).message);
      if (!listing && path) {
        // A typed path that does not work: start from home instead.
        try {
          const l = await api.folders();
          setListing(l);
          onChange(l.path);
        } catch {
          // The first error already says what is wrong.
        }
      }
    } finally {
      setLoading(false);
    }
  }

  useEffect(() => {
    void go(start || undefined);
    // Only on open.
  }, []);

  const crumbs = listing ? crumbsOf(listing.path) : [];

  return (
    <div className="flex flex-col gap-2.5">
      <div className="flex flex-wrap items-center gap-1 text-[13.5px]">
        <Button variant="link" size="sm" className="px-1.5" onClick={() => void go(listing?.home)} aria-label="Home folder">
          <House className="size-3.5" />
        </Button>
        {crumbs.map((c, i) => (
          <span key={c.path} className="flex items-center gap-1">
            {i > 0 ? <ChevronRight className="size-3 text-faint" /> : null}
            <button
              type="button"
              onClick={() => void go(c.path)}
              className={cn("cursor-pointer rounded px-1 py-0.5 font-mono text-[13px] hover:bg-sunken", i === crumbs.length - 1 ? "font-semibold text-ink" : "text-muted")}
            >
              {c.name}
            </button>
          </span>
        ))}
        {loading ? <LoaderCircle className="ml-1 size-3.5 animate-spin text-faint" /> : null}
      </div>

      {listing?.is_git ? (
        <div className="flex items-center gap-2 text-[13.5px] text-muted">
          <i className="size-[7px] rounded-full bg-ok" />
          This folder is a git repository.
        </div>
      ) : null}

      <div className="max-h-[300px] min-h-[180px] overflow-auto rounded-lg border border-line-2 bg-surface" role="list" aria-label="Folders">
        {listing?.parent ? (
          <button type="button" onClick={() => void go(listing.parent!)} className="flex w-full cursor-pointer items-center gap-2.5 border-b border-line px-3 py-2 text-left text-muted hover:bg-sunken">
            <CornerLeftUp className="size-4" />
            Up one folder
          </button>
        ) : null}
        {listing?.entries.map((e) => (
          <div key={e.path} role="listitem" className="group flex items-center border-b border-line last:border-b-0 hover:bg-sunken">
            <button type="button" onClick={() => void go(e.path)} className="flex min-w-0 flex-1 cursor-pointer items-center gap-2.5 px-3 py-2 text-left">
              <Folder className="size-4 shrink-0 text-faint" />
              <span className="truncate text-ink">{e.name}</span>
              {e.is_git ? (
                <span className="flex shrink-0 items-center gap-1.5 text-[12.5px] text-muted">
                  <i className="size-[7px] rounded-full bg-ok" />
                  git repo
                </span>
              ) : null}
            </button>
            {e.is_git ? (
              <Button size="sm" className="mr-2" onClick={() => onPick(e.path)}>
                Choose
              </Button>
            ) : null}
          </div>
        ))}
        {listing && listing.entries.length === 0 ? <div className="px-3 py-6 text-center text-[13.5px] text-faint">No folders here.</div> : null}
        {listing?.truncated ? <div className="px-3 py-2 text-[12.5px] text-faint">Showing the first 500 folders.</div> : null}
      </div>

      {error ? <span className="text-[13px] text-bad">{error}</span> : null}
    </div>
  );
}

function crumbsOf(path: string): { name: string; path: string }[] {
  const parts = path.split("/").filter(Boolean);
  const out = [{ name: "/", path: "/" }];
  let acc = "";
  for (const p of parts) {
    acc += `/${p}`;
    out.push({ name: p, path: acc });
  }
  return out;
}
