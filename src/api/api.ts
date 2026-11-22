import type {
  Bootstrap, EntryGroup, FavoriteToggle, FtsPage, HistoryItem, Settings, SyncExportReport, SyncImportReport, SyncInfo, Theme, WordHit, WordRef,
} from "../types";

/** مرز میان رابط و هسته (Tauri یا نمونهٔ آزمایشی). همهٔ جست‌وجوها در Rust انجام می‌شود. */
export interface Api {
  bootstrap(): Promise<Bootstrap>;
  searchWords(query: string): Promise<WordHit[]>;
  searchFulltext(query: string, offset: number): Promise<FtsPage>;
  getEntryGroup(key: string, opts?: { focusId?: number; offset?: number }): Promise<EntryGroup | null>;
  recordVisit(ref: WordRef): Promise<HistoryItem[]>;
  clearHistory(): Promise<HistoryItem[]>;
  toggleFavorite(ref: WordRef): Promise<FavoriteToggle>;
  saveSettings(settings: Settings): Promise<Settings>;
  setWindowTheme(theme: Theme): Promise<void>;
  // همگام‌سازی دستی علاقه‌مندی‌ها و تاریخچه از راه یک پوشه (مثلاً پوشهٔ Google Drive)
  /** پنجرهٔ انتخاب پوشه؛ null = کاربر منصرف شد */
  pickSyncFolder(): Promise<string | null>;
  getSyncInfo(): Promise<SyncInfo>;
  /** پوشه را به‌خاطر می‌سپارد (null = فراموش کن) */
  setSyncDir(dir: string | null): Promise<SyncInfo>;
  /** ذخیرهٔ خودکار پس از هر تغییر را روشن/خاموش می‌کند */
  setAutoExport(enabled: boolean): Promise<SyncInfo>;
  syncExport(): Promise<SyncExportReport>;
  syncImport(): Promise<SyncImportReport>;
}

/** پیامی که هسته وقتی جست‌وجوی تازه‌تری جست‌وجوی قبلی را قطع کند می‌فرستد */
export const CANCELLED = "cancelled";

export function isCancelled(e: unknown): boolean {
  return e === CANCELLED || (e instanceof Error && e.message === CANCELLED) || String(e) === CANCELLED;
}

export function describeError(e: unknown): string {
  const text = e instanceof Error ? e.message : String(e);
  return text || "خطای ناشناخته";
}
