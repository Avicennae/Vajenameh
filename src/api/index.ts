import { DEFAULT_SETTINGS } from "../types";
import type { Api } from "./api";
import { tauriApi } from "./tauri";

export type { Api } from "./api";
export { CANCELLED, describeError, isCancelled } from "./api";

export function isTauri(): boolean {
  return typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;
}

const NOT_IN_TAURI =
  "این صفحه بیرون از پوستهٔ Tauri باز شده است و به هستهٔ جست‌وجو دسترسی ندارد. " +
  "برنامه را با «npm run tauri dev» اجرا کنید (یا برای دیدن ظاهر، با «VITE_MOCK=1 npm run dev» و داده‌های نمونه).";

/** وقتی نه Tauri هست و نه حالت نمونه: فقط صفحهٔ راهنما نشان داده می‌شود */
const unavailableApi: Api = {
  bootstrap: async () => ({ info: null, dbError: NOT_IN_TAURI, settings: DEFAULT_SETTINGS, history: [], favorites: [], warnings: [] }),
  searchWords: () => Promise.reject(NOT_IN_TAURI),
  searchFulltext: () => Promise.reject(NOT_IN_TAURI),
  getEntryGroup: () => Promise.reject(NOT_IN_TAURI),
  recordVisit: () => Promise.reject(NOT_IN_TAURI),
  clearHistory: () => Promise.reject(NOT_IN_TAURI),
  toggleFavorite: () => Promise.reject(NOT_IN_TAURI),
  saveSettings: () => Promise.reject(NOT_IN_TAURI),
  setWindowTheme: async () => undefined,
  pickSyncFolder: () => Promise.reject(NOT_IN_TAURI),
  getSyncInfo: () => Promise.reject(NOT_IN_TAURI),
  setSyncDir: () => Promise.reject(NOT_IN_TAURI),
  setAutoExport: () => Promise.reject(NOT_IN_TAURI),
  syncExport: () => Promise.reject(NOT_IN_TAURI),
  syncImport: () => Promise.reject(NOT_IN_TAURI),
};

export async function getApi(): Promise<Api> {
  if (isTauri()) return tauriApi;
  // با VITE_MOCK نبودن، این شاخه در بیلد نهایی حذف می‌شود (داده‌های نمونه همراه نصب‌کننده نمی‌شوند)
  if (import.meta.env.VITE_MOCK) {
    const { mockApi } = await import("./mock");
    return mockApi;
  }
  return unavailableApi;
}
