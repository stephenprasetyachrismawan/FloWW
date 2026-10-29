/** Text size and font, kept per computer (browser storage). */
export type Appearance = { size: "s" | "m" | "l" | "xl"; font: "sans" | "serif" | "system" };

const KEY = "floww.appearance";
export const SIZES: { value: Appearance["size"]; label: string; zoom: number }[] = [
  { value: "s", label: "Small", zoom: 0.92 },
  { value: "m", label: "Normal", zoom: 1 },
  { value: "l", label: "Large", zoom: 1.1 },
  { value: "xl", label: "Extra large", zoom: 1.22 },
];
export const FONTS: { value: Appearance["font"]; label: string; why: string }[] = [
  { value: "sans", label: "Sans", why: "Clean, the default" },
  { value: "serif", label: "Serif", why: "Book-like, easy for long reading" },
  { value: "system", label: "System", why: "Your computer's own font" },
];

export function loadAppearance(): Appearance {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "{}");
    return { size: v.size ?? "m", font: v.font ?? "sans" };
  } catch {
    return { size: "m", font: "sans" };
  }
}

export function applyAppearance(a: Appearance) {
  const root = document.documentElement;
  const zoom = SIZES.find((s) => s.value === a.size)?.zoom ?? 1;
  (document.getElementById("root") as HTMLElement | null)?.style.setProperty("zoom", String(zoom));
  if (a.font === "sans") root.removeAttribute("data-font");
  else root.setAttribute("data-font", a.font);
}

export function saveAppearance(a: Appearance) {
  applyAppearance(a);
  try {
    localStorage.setItem(KEY, JSON.stringify(a));
  } catch {
    // Still applies for this session.
  }
}
