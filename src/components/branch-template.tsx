import { useEffect, useState } from "react";

import { Field, Input } from "@/components/ui/field";
import { api } from "@/lib/api";

export const DEFAULT_BRANCH_TEMPLATE = "dev/{ddmmyyyy}_{slug}";

/** Template for new development branch names, with a live example from the server. */
export function BranchTemplateField({
  id,
  value,
  onChange,
  initials,
  onValid,
}: {
  id: string;
  value: string;
  onChange: (v: string) => void;
  initials?: string;
  onValid?: (ok: boolean) => void;
}) {
  const [preview, setPreview] = useState<{ name?: string; error?: string }>({});

  useEffect(() => {
    const t = window.setTimeout(() => {
      api
        .branchPreview(value, initials)
        .then((r) => {
          setPreview({ name: r.name });
          onValid?.(true);
        })
        .catch((e: Error) => {
          setPreview({ error: e.message });
          onValid?.(false);
        });
    }, 250);
    return () => window.clearTimeout(t);
    // onValid is a setter from the parent; it does not change.
  }, [value, initials]);

  return (
    <Field
      label="New branch names"
      htmlFor={id}
      hint={
        <>
          Use <code className="font-mono">{"{slug}"}</code> <code className="font-mono">{"{type}"}</code> <code className="font-mono">{"{prd_id}"}</code>{" "}
          <code className="font-mono">{"{ddmmyyyy}"}</code> <code className="font-mono">{"{yyyymmdd}"}</code>.{" "}
          {value.trim() !== DEFAULT_BRANCH_TEMPLATE ? (
            <button type="button" className="cursor-pointer underline hover:text-ink" onClick={() => onChange(DEFAULT_BRANCH_TEMPLATE)}>
              Reset to {DEFAULT_BRANCH_TEMPLATE}
            </button>
          ) : null}
        </>
      }
    >
      <Input id={id} className="font-mono text-[13.5px]" value={value} onChange={(e) => onChange(e.target.value)} spellCheck={false} />
      {preview.error ? (
        <span className="text-[13px] text-bad">{preview.error}</span>
      ) : preview.name ? (
        <span className="text-[13px] text-muted">
          A request today becomes <span className="font-mono text-ink">{preview.name}</span>
        </span>
      ) : null}
    </Field>
  );
}
