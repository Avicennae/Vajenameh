import type { EntryGroup } from "../types";

/** HTML (پاک‌سازی‌شدهٔ معنی) → متن خالص با حفظ شکست خط‌ها */
export function htmlToPlainText(html: string): string {
  const doc = new DOMParser().parseFromString(html.replace(/<br\s*\/?>/gi, "\n"), "text/html");
  return (doc.body.textContent ?? "")
    .replace(/[ \t\u00a0]+\n/g, "\n")
    .replace(/\n[ \t]+/g, "\n")
    .replace(/\n{3,}/g, "\n\n")
    .trim();
}

/** متن قابل کپی برای صفحهٔ جاری گروه */
export function groupToPlainText(group: EntryGroup): string {
  const parts = group.entries.map((e) => htmlToPlainText(e.html));
  return parts.join("\n\n———\n\n");
}
