const faNumbers = new Intl.NumberFormat("fa-IR");

/** عدد با ارقام و جداکنندهٔ فارسی: ۳۱۲٬۵۰۷ */
export function faNumber(n: number): string {
  return faNumbers.format(n);
}

export function formatDate(iso: string): string {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleDateString("fa-IR", { year: "numeric", month: "long", day: "numeric" });
}

export function shortCommit(hash: string): string {
  return hash && hash !== "unknown" ? hash.slice(0, 7) : "نامشخص";
}
