import { Page, PageHeader } from "@/components/page";

/** Pages that exist in the design but are built in later changes. */
export function PlaceholderPage({ title, about }: { title: string; about: string }) {
  return (
    <Page narrow>
      <PageHeader title={title} sub={about} />
      <p className="text-[13.5px] text-faint">Not built yet. See docs/DESIGN-DECISIONS.md and docs/prototype for the planned screen.</p>
    </Page>
  );
}
