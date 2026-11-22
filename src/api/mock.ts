// فقط برای توسعه و تست مرورگری (VITE_MOCK=1): چند مدخل واقعی که از پایگاه واقعی و با sanitizer واقعی
// بیرون کشیده شده (crates/dehkhoda-core/examples/export_mock.rs). منطق جست‌وجو اینجا ساده‌شده است؛
// جست‌وجوی واقعی در Rust است و تست می‌شود.
import data from "./mockData.json";
import type { Api } from "./api";
import type { DbInfo, EntryGroup, FavoriteItem, FtsHit, HistoryItem, Segment, Settings, SyncInfo, WordHit } from "../types";
import { DEFAULT_SETTINGS } from "../types";

const groups = data as EntryGroup[];
const PAGE = 40;

const simplify = (s: string) =>
  s
    .replace(/[\u064B-\u065F\u0670\u0640]/g, "")
    .replace(/[يى]/g, "ی")
    .replace(/ك/g, "ک")
    .replace(/[\u200c\u00a0]/g, " ")
    .replace(/[آأإٱ]/g, "ا")
    .replace(/\s+/g, " ")
    .trim()
    .toLowerCase();

const plain = (html: string) => simplify(html.replace(/<[^>]*>/g, " "));

const info: DbInfo = {
  schemaVersion: 2, normVersion: 1, hasFts: true, rowCount: 312507, distinctWords: 215122,
  sourceUrl: "https://github.com/nimah79/Dehkhoda-SQL", sourceCommit: "185ed68bc6ad7c4244c5f4cc1c9968aa8aefa0f4", builtAt: "2026-11-16T10:03:23Z",
};

let history: HistoryItem[] = [];
let favorites: FavoriteItem[] = [];
let settings: Settings = { ...DEFAULT_SETTINGS };
let syncDir: string | null = null;
let autoExport = true;

const MOCK_SYNC_FILE = "dehkhoda-sync-DESKTOP-DEMO-1a2b3c4d.json";
const syncInfo = (): SyncInfo => ({ dir: syncDir, deviceName: "DESKTOP-DEMO", fileName: MOCK_SYNC_FILE, autoExport, lastAuto: null });
const NO_SYNC_DIR = "هنوز پوشه‌ای برای همگام‌سازی انتخاب نشده است";

export const mockApi: Api = {
  async bootstrap() {
    return { info, dbError: null, settings, history, favorites, warnings: [] };
  },
  async searchWords(query) {
    const q = simplify(query);
    if (!q) return [];
    const rank = (g: EntryGroup) => {
      const n = simplify(g.wordNorm);
      return n === q ? 0 : n.startsWith(q) ? 1 : n.includes(q) ? 2 : 9;
    };
    const kinds = ["exact", "prefix", "contains"] as const;
    return groups
      .map((g) => ({ g, r: rank(g) }))
      .filter((x) => x.r < 9)
      .sort((a, b) => a.r - b.r || a.g.wordNorm.length - b.g.wordNorm.length)
      .slice(0, 50)
      .map<WordHit>(({ g, r }) => ({ id: g.entries[0].id, word: g.word, wordNorm: g.wordNorm, count: g.total, kind: kinds[r] }));
  },
  async searchFulltext(query, offset) {
    const tokens = simplify(query).split(" ").filter(Boolean);
    const hits: FtsHit[] = [];
    for (const g of groups) {
      for (const e of g.entries) {
        const text = plain(e.html);
        if (tokens.every((t) => text.includes(t))) {
          const at = Math.max(0, text.indexOf(tokens[0]) - 40);
          const slice = text.slice(at, at + 160);
          const snippet: Segment[] = [{ text: (at ? "… " : "") + slice, hit: false }];
          hits.push({ id: e.id, word: e.word, wordNorm: g.wordNorm, snippet });
        }
      }
    }
    return { hits: hits.slice(offset, offset + 50), hasMore: hits.length > offset + 50 };
  },
  async getEntryGroup(key, opts) {
    const k = simplify(key);
    const g = groups.find((x) => simplify(x.wordNorm) === k);
    if (!g) return null;
    let offset = Math.floor((opts?.offset ?? 0) / PAGE) * PAGE;
    if (opts?.focusId !== undefined) {
      const pos = g.entries.findIndex((e) => e.id === opts.focusId);
      if (pos >= 0) offset = Math.floor(pos / PAGE) * PAGE;
    }
    return { ...g, offset, pageSize: PAGE, entries: g.entries.slice(offset, offset + PAGE) };
  },
  async recordVisit(ref) {
    history = [{ ...ref, at: Date.now() }, ...history.filter((h) => h.wordNorm !== ref.wordNorm)];
    return history;
  },
  async clearHistory() {
    history = [];
    return history;
  },
  async toggleFavorite(ref) {
    const was = favorites.some((f) => f.wordNorm === ref.wordNorm);
    favorites = was ? favorites.filter((f) => f.wordNorm !== ref.wordNorm) : [{ ...ref, at: Date.now() }, ...favorites];
    return { isFavorite: !was, favorites };
  },
  async saveSettings(next) {
    settings = next;
    return settings;
  },
  async setWindowTheme() {},
  // همگام‌سازی: در حالت نمونه فقط ظاهر آزمایش می‌شود (هیچ فایلی نوشته یا خوانده نمی‌شود)
  async pickSyncFolder() {
    return "C:\\Users\\Demo\\Google Drive\\Dehkhoda";
  },
  async getSyncInfo() {
    return syncInfo();
  },
  async setSyncDir(dir) {
    syncDir = dir;
    return syncInfo();
  },
  async setAutoExport(enabled) {
    autoExport = enabled;
    return syncInfo();
  },
  async syncExport() {
    if (!syncDir) throw new Error(NO_SYNC_DIR);
    return { dir: syncDir, file: MOCK_SYNC_FILE, favorites: favorites.length, history: history.length };
  },
  async syncImport() {
    if (!syncDir) throw new Error(NO_SYNC_DIR);
    return { dir: syncDir, filesRead: 0, skipped: [], favoritesAdded: 0, favoritesRemoved: 0, historyAdded: 0, historyRemoved: 0, favorites, history };
  },
};
