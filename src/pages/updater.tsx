import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";

import { UpdateProgressView } from "@/components/update";
import { Button } from "@/components/ui/button";
import type { UpdateProgress } from "@/lib/types";

/** The small window that installs an update after the main app closed. */
export function Updater() {
  const [p, setP] = useState<UpdateProgress>({ stage: "check", percent: null, message: "Looking for the new version" });
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void invoke<UpdateProgress>("updater_state").then(setP).catch(() => {});
    const off = listen<UpdateProgress>("update-progress", (e) => setP(e.payload));
    return () => {
      void off.then((f) => f());
    };
  }, []);

  async function openOld() {
    try {
      await invoke("updater_open_old");
    } catch (e) {
      setError((e as Error).message ?? String(e));
    }
  }

  return (
    <div className="flex h-screen flex-col gap-4 overflow-hidden bg-window px-7 py-6">
      <h1 className="font-serif text-[21px] leading-tight font-semibold">{p.stage === "failed" ? "The update did not finish" : "Updating FloWW"}</h1>
      <UpdateProgressView p={p} />
      {error ? <p className="text-[13px] text-bad">{error}</p> : null}
      {p.stage === "failed" ? (
        <div className="mt-auto flex justify-end gap-3">
          <Button variant="link" onClick={() => void getCurrentWindow().close()}>
            Close
          </Button>
          <Button variant="primary" onClick={openOld}>
            Open FloWW
          </Button>
        </div>
      ) : null}
    </div>
  );
}
