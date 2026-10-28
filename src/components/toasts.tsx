import { useStore } from "@/lib/store";
import { cn } from "@/lib/utils";

const DOT = { you: "bg-you", ok: "bg-ok", bad: "bg-bad", info: "bg-ai" };

export function Toasts() {
  const { toasts } = useStore();
  return (
    <div aria-live="polite" className="pointer-events-none fixed right-4.5 bottom-4.5 z-50 flex w-[min(340px,calc(100%-36px))] flex-col gap-2">
      {toasts.map((t) => (
        <div key={t.id} className="pointer-events-auto flex items-start gap-3 rounded-[10px] bg-ink px-3.5 py-3 text-[13.5px] text-window shadow-pop">
          <i className={cn("mt-[7px] size-[7px] shrink-0 rounded-full", DOT[t.kind])} />
          <div className="min-w-0">
            <b className="block font-semibold">{t.title}</b>
            {t.body ? <small className="block break-words whitespace-pre-wrap opacity-70">{t.body}</small> : null}
          </div>
        </div>
      ))}
    </div>
  );
}
