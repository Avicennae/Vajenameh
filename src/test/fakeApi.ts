import { vi } from "vitest";
import type { Api } from "../api/api";
import type { Bootstrap, DbInfo, EntryGroup, FtsHit, Settings, SyncExportReport, SyncImportReport, SyncInfo, WordHit } from "../types";
import { DEFAULT_SETTINGS } from "../types";

export const INFO: DbInfo = {
  schemaVersion: 2, normVersion: 1, hasFts: true, rowCount: 312507, distinctWords: 215122,
  sourceUrl: "https://github.com/nimah79/Dehkhoda-SQL", sourceCommit: "185ed68bc6ad7c4244c5f4cc1c9968aa8aefa0f4", builtAt: "2026-11-16T10:03:23Z",
};

export const WORDS: { word: string; count: number }[] = [
  { word: "آب", count: 3 }, { word: "آباد", count: 1 }, { word: "آبادان", count: 1 }, { word: "کتاب", count: 1 },
];

export function makeGroup(word: string, total = 1, offset = 0, pageSize = 40): EntryGroup {
  const n = Math.min(pageSize, total - offset);
  return {
    wordNorm: word, word, total, offset, pageSize,
    entries: Array.from({ length: n }, (_, i) => ({
      id: 100 + offset + i, word,
      html: `<span class="hlight">${word} .</span> معنی شمارهٔ ${offset + i + 1}<br><font color="red">سعدی .</font>`,
    })),
  };
}

export function bootstrap(over: Partial<Bootstrap> = {}): Bootstrap {
  return { info: INFO, dbError: null, settings: { ...DEFAULT_SETTINGS }, history: [], favorites: [], warnings: [], ...over };
}

/** یک Api درون‌حافظه‌ای که همهٔ فراخوانی‌ها را می‌شود با vi.fn بازرسی کرد */
export function makeApi(over: Partial<Api> = {}) {
  let history: Bootstrap["history"] = [];
  let favorites: Bootstrap["favorites"] = [];
  let syncDir: string | null = null;
  let autoExport = true;
  const syncInfo = (): SyncInfo => ({ dir: syncDir, deviceName: "PC-A", fileName: "dehkhoda-sync-PC-A-1a2b3c4d.json", autoExport, lastAuto: null });
  const api = {
    bootstrap: vi.fn(async () => bootstrap()),
    searchWords: vi.fn(async (q: string): Promise<WordHit[]> =>
      WORDS.filter((w) => w.word.startsWith(q)).map((w, i) => ({ id: i + 1, word: w.word, wordNorm: w.word, count: w.count, kind: "prefix" as const }))),
    searchFulltext: vi.fn(async (q: string, offset: number) => {
      const hits: FtsHit[] = [
        { id: 205, word: "آسیاب", wordNorm: "آسیاب", snippet: [{ text: "جای آرد کردن ", hit: false }, { text: q, hit: true }, { text: " و گندم", hit: false }] },
        { id: 206, word: "کتابخانه", wordNorm: "کتابخانه", snippet: [{ text: q, hit: true }] },
      ];
      return { hits: hits.slice(offset), hasMore: false };
    }),
    getEntryGroup: vi.fn(async (key: string, opts?: { focusId?: number; offset?: number }) => makeGroup(key, key === "آب" ? 100 : 1, opts?.offset ?? 0)),
    recordVisit: vi.fn(async (ref: { wordNorm: string; word: string }) => {
      history = [{ ...ref, at: Date.now() }, ...history.filter((h) => h.wordNorm !== ref.wordNorm)];
      return history;
    }),
    clearHistory: vi.fn(async () => (history = [])),
    toggleFavorite: vi.fn(async (ref: { wordNorm: string; word: string }) => {
      const was = favorites.some((f) => f.wordNorm === ref.wordNorm);
      favorites = was ? favorites.filter((f) => f.wordNorm !== ref.wordNorm) : [{ ...ref, at: Date.now() }, ...favorites];
      return { isFavorite: !was, favorites };
    }),
    saveSettings: vi.fn(async (s: Settings) => s),
    setWindowTheme: vi.fn(async () => undefined),
    pickSyncFolder: vi.fn(async (): Promise<string | null> => "/home/me/Drive/Dehkhoda"),
    getSyncInfo: vi.fn(async () => syncInfo()),
    setSyncDir: vi.fn(async (dir: string | null) => {
      syncDir = dir;
      return syncInfo();
    }),
    setAutoExport: vi.fn(async (enabled: boolean) => {
      autoExport = enabled;
      return syncInfo();
    }),
    syncExport: vi.fn(async (): Promise<SyncExportReport> => ({ dir: syncDir ?? "", file: syncInfo().fileName, favorites: favorites.length, history: history.length })),
    syncImport: vi.fn(async (): Promise<SyncImportReport> => ({
      dir: syncDir ?? "", filesRead: 1, skipped: [], favoritesAdded: 0, favoritesRemoved: 0, historyAdded: 0, historyRemoved: 0, favorites, history,
    })),
    ...over,
  } satisfies Api;
  return api;
}
