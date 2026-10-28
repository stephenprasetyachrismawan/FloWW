import { useState } from "react";
import { useNavigate } from "react-router";

import { Button } from "@/components/ui/button";
import { Field, Input, Segmented, Textarea } from "@/components/ui/field";
import { Modal } from "@/components/ui/modal";
import { api } from "@/lib/api";
import { useAction, useStore } from "@/lib/store";
import type { PrdType } from "@/lib/types";

export function NewRequest({ onClose }: { onClose: () => void }) {
  const { wsId } = useStore();
  const { run, busy } = useAction();
  const navigate = useNavigate();
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [kind, setKind] = useState<PrdType>("bug");
  const [reporter, setReporter] = useState("");

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    if (!wsId) return;
    const r = await run(() => api.createPrd(wsId, { title, description, kind, reporter }));
    if (r) {
      onClose();
      navigate(`/prd/${r.id}`);
    }
  }

  return (
    <form onSubmit={submit}>
      <Modal
        title="New request"
        sub="Describe it like you would to a colleague. The AI reads the brain, database and code, then asks what it still needs."
        onClose={onClose}
        footer={
          <>
            <span className="flex-1" />
            <Button type="button" variant="link" onClick={onClose}>
              Cancel
            </Button>
            <Button type="submit" variant="primary" disabled={busy || !title.trim()}>
              Create request
            </Button>
          </>
        }
      >
        <Field label="What is it about?" htmlFor="nr-title">
          <Input id="nr-title" value={title} onChange={(e) => setTitle(e.target.value)} placeholder="Invoice muncul dua kali untuk pengiriman parsial" required />
        </Field>
        <Field label="Details" htmlFor="nr-desc">
          <Textarea id="nr-desc" value={description} onChange={(e) => setDescription(e.target.value)} placeholder="What happened, where, and what should happen instead" />
        </Field>
        <div className="flex flex-col gap-1.5">
          <span className="text-[13.5px] font-semibold text-ink">Kind</span>
          <Segmented
            label="Kind"
            value={kind}
            onChange={setKind}
            options={[
              { value: "bug", label: "Something is broken" },
              { value: "improve", label: "Make it better" },
              { value: "feature", label: "Something new" },
            ]}
          />
        </div>
        <Field label={<>Who asked? <span className="font-normal text-faint">optional</span></>} htmlFor="nr-rep">
          <Input id="nr-rep" value={reporter} onChange={(e) => setReporter(e.target.value)} placeholder="Dewi, Finance" />
        </Field>
      </Modal>
    </form>
  );
}
