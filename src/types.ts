// نوع‌های مشترک میان رابط و دستورهای Tauri (JSON به‌صورت camelCase؛ مطابق serde در Rust)

export type MatchKind = "exact" | "prefix" | "contains";

export interface WordHit {
  id: number;
  word: string;
  wordNorm: string;
  /** تعداد مدخل‌های هم‌املا */
  count: number;
  kind: MatchKind;
}

export interface Segment {
  text: string;
  hit: boolean;
}

export interface FtsHit {
  id: number;
  word: string;
  wordNorm: string;
  snippet: Segment[];
}

export interface FtsPage {
  hits: FtsHit[];
  hasMore: boolean;
}

export interface Entry {
  id: number;
  word: string;
  /** HTML پاک‌سازی‌شده توسط هستهٔ Rust */
  html: string;
}

export interface EntryGroup {
  wordNorm: string;
  word: string;
  /** تعداد کل مدخل‌های هم‌املا */
  total: number;
  /** اندیس نخستین مدخل این صفحه */
  offset: number;
  pageSize: number;
  entries: Entry[];
}

export interface DbInfo {
  schemaVersion: number;
  normVersion: number;
  hasFts: boolean;
  rowCount: number;
  distinctWords: number;
  sourceUrl: string;
  sourceCommit: string;
  builtAt: string;
}

export interface WordRef {
  wordNorm: string;
  word: string;
}

export interface HistoryItem extends WordRef {
  at: number;
}

export interface FavoriteItem extends WordRef {
  at: number;
}

export type Theme = "system" | "light" | "dark";

export interface Settings {
  theme: Theme;
  fontScale: number;
  fulltext: boolean;
}

/** همهٔ وضعیت اولیه در یک فراخوانی */
export interface Bootstrap {
  info: DbInfo | null;
  /** اگر دیتابیس باز نشد، پیام خطا (رابط صفحهٔ راهنما نشان می‌دهد) */
  dbError: string | null;
  settings: Settings;
  history: HistoryItem[];
  favorites: FavoriteItem[];
  warnings: string[];
}

export interface FavoriteToggle {
  isFavorite: boolean;
  favorites: FavoriteItem[];
}

/** همگام‌سازی دستی از راه یک پوشه (مثلاً پوشه‌ای داخل Google Drive) */
/** نتیجهٔ آخرین ذخیرهٔ خودکار (فقط تا بسته شدن برنامه در حافظه است) */
export interface AutoStatus {
  /** میلی‌ثانیهٔ یونیکس */
  at: number;
  /** null = موفق */
  error: string | null;
}

export interface SyncInfo {
  /** پوشهٔ انتخاب‌شده؛ null یعنی هنوز انتخاب نشده */
  dir: string | null;
  deviceName: string;
  /** نام فایلی که همین رایانه در آن پوشه می‌نویسد */
  fileName: string;
  /** ذخیرهٔ خودکار پس از هر تغییر روشن است؟ (پیش‌فرض: بله) */
  autoExport: boolean;
  /** نتیجهٔ آخرین ذخیرهٔ خودکار در همین اجرای برنامه؛ null = هنوز انجام نشده */
  lastAuto: AutoStatus | null;
}

export interface SyncExportReport {
  dir: string;
  file: string;
  /** تعداد علاقه‌مندی‌ها و واژه‌های تاریخچه که در فایل نوشته شد */
  favorites: number;
  history: number;
}

export interface SkippedFile {
  file: string;
  reason: string;
}

export interface SyncImportReport {
  dir: string;
  /** چند فایل از رایانه‌های دیگر خوانده شد */
  filesRead: number;
  /** فایل‌هایی که خوانده نشد، هر کدام با دلیلش */
  skipped: SkippedFile[];
  favoritesAdded: number;
  favoritesRemoved: number;
  historyAdded: number;
  historyRemoved: number;
  /** فهرست‌های تازهٔ همین رایانه پس از ادغام */
  favorites: FavoriteItem[];
  history: HistoryItem[];
}

export const DEFAULT_SETTINGS: Settings = { theme: "system", fontScale: 1, fulltext: false };
export const MIN_FONT_SCALE = 0.8;
export const MAX_FONT_SCALE = 2;
