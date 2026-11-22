import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type { Api } from "./api";

/** پیاده‌سازی واقعی: هر متد یک دستور Rust (src-tauri/src/lib.rs) را صدا می‌زند. */
export const tauriApi: Api = {
  bootstrap: () => invoke("bootstrap"),
  searchWords: (query) => invoke("search_words", { query }),
  searchFulltext: (query, offset) => invoke("search_fulltext", { query, offset }),
  getEntryGroup: (key, opts) => invoke("get_entry_group", { key, focusId: opts?.focusId ?? null, offset: opts?.offset ?? 0 }),
  recordVisit: (ref) => invoke("record_visit", { wordNorm: ref.wordNorm, word: ref.word }),
  clearHistory: () => invoke("clear_history"),
  toggleFavorite: (ref) => invoke("toggle_favorite", { wordNorm: ref.wordNorm, word: ref.word }),
  saveSettings: (settings) => invoke("save_settings", { settings }),
  pickSyncFolder: () => invoke("pick_sync_folder"),
  getSyncInfo: () => invoke("get_sync_info"),
  setSyncDir: (dir) => invoke("set_sync_dir", { dir }),
  setAutoExport: (enabled) => invoke("set_auto_export", { enabled }),
  syncExport: () => invoke("sync_export"),
  syncImport: () => invoke("sync_import"),
  // رنگ نوار عنوان ویندوز با تم برنامه هماهنگ شود (system = پیروی از سیستم)
  setWindowTheme: async (theme) => {
    await getCurrentWindow().setTheme(theme === "system" ? null : theme);
  },
};
