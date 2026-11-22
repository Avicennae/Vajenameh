import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { describeError, isCancelled } from "../api/api";
import type { Api } from "../api/api";
import { moveIndex } from "../lib/listNav";
import type {
  Bootstrap, EntryGroup, FavoriteItem, FtsHit, HistoryItem, Segment, Settings, WordRef,
} from "../types";
import { MAX_FONT_SCALE, MIN_FONT_SCALE } from "../types";

export type Mode = "word" | "fulltext";
export type Tab = "results" | "history" | "favorites";
export type SearchStatus = "idle" | "loading" | "done" | "error" | "short";

/** یک ردیف در فهرست کناری (نتیجه، تاریخچه یا علاقه‌مندی) */
export interface ListItem {
  key: string;
  word: string;
  wordNorm: string;
  /** برای نتایج متن‌کامل: مدخل هدف داخل گروه */
  focusId?: number;
  /** تعداد مدخل‌های هم‌املا (فقط جست‌وجوی واژه) */
  count?: number;
  snippet?: Segment[];
}

export interface DictionaryOptions {
  debounceWordMs?: number;
  debounceFtsMs?: number;
}

/** حداقل طول ورودی برای جست‌وجوی متن‌کامل (تک‌حرفی‌ها تقریباً همهٔ مدخل‌ها را برمی‌گردانند) */
export const MIN_FTS_CHARS = 2;
const FONT_STEP = 0.1;

const clampScale = (v: number) => Math.round(Math.min(MAX_FONT_SCALE, Math.max(MIN_FONT_SCALE, v)) * 100) / 100;

const ftsItem = (h: FtsHit): ListItem => ({
  key: `f${h.id}`, word: h.word, wordNorm: h.wordNorm, focusId: h.id, snippet: h.snippet,
});

export function useDictionary(api: Api, boot: Bootstrap, options: DictionaryOptions = {}) {
  const { debounceWordMs = 90, debounceFtsMs = 400 } = options;
  const ftsAvailable = !!boot.info?.hasFts;

  const [query, setQuery] = useState("");
  const [mode, setModeState] = useState<Mode>(boot.settings.fulltext && ftsAvailable ? "fulltext" : "word");
  const [results, setResults] = useState<ListItem[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [status, setStatus] = useState<SearchStatus>("idle");
  const [tab, setTabState] = useState<Tab>("results");
  const [activeIndex, setActiveIndex] = useState(0);
  const [group, setGroup] = useState<EntryGroup | null>(null);
  const [focusEntryId, setFocusEntryId] = useState<number | null>(null);
  const [groupLoading, setGroupLoading] = useState(false);
  const [history, setHistory] = useState<HistoryItem[]>(boot.history);
  const [favorites, setFavorites] = useState<FavoriteItem[]>(boot.favorites);
  const [settings, setSettings] = useState<Settings>(boot.settings);
  const [notice, setNotice] = useState<{ id: number; text: string } | null>(null);

  const settingsRef = useRef(settings);
  settingsRef.current = settings;
  const searchSeq = useRef(0);
  const openSeq = useRef(0);
  const loadingMore = useRef(false);
  /** Enter قبل از رسیدن نتایج: وقتی نتایج آمد، اولین نتیجه باز شود */
  const pendingOpen = useRef(false);

  const openRef = useRef<(item: ListItem) => Promise<void>>(async () => undefined);
  const say = useCallback((text: string) => setNotice({ id: Date.now() + Math.random(), text }), []);
  const dismissNotice = useCallback(() => setNotice(null), []);

  // هشدارهای زمان راه‌اندازی (مثلاً فایل تاریخچهٔ خراب)
  useEffect(() => {
    if (boot.warnings.length) say(boot.warnings[0]);
  }, [boot.warnings, say]);

  // ---------------- جست‌وجو با debounce ----------------
  useEffect(() => {
    const q = query.trim();
    const seq = ++searchSeq.current;
    pendingOpen.current = false;
    if (!q) {
      setResults([]);
      setHasMore(false);
      setStatus("idle");
      return;
    }
    if (mode === "fulltext" && [...q].length < MIN_FTS_CHARS) {
      setResults([]);
      setHasMore(false);
      setStatus("short");
      return;
    }
    setStatus("loading");
    const timer = setTimeout(async () => {
      let first: ListItem | undefined;
      try {
        if (mode === "word") {
          const hits = await api.searchWords(q);
          if (seq !== searchSeq.current) return;
          const list = hits.map((h): ListItem => ({ key: `w${h.id}`, word: h.word, wordNorm: h.wordNorm, count: h.count }));
          setResults(list);
          setHasMore(false);
          first = list[0];
        } else {
          const page = await api.searchFulltext(q, 0);
          if (seq !== searchSeq.current) return;
          const list = page.hits.map(ftsItem);
          setResults(list);
          setHasMore(page.hasMore);
          first = list[0];
        }
        setActiveIndex(0);
        setStatus("done");
        if (pendingOpen.current) {
          pendingOpen.current = false;
          if (first) void openRef.current(first);
        }
      } catch (e) {
        if (seq !== searchSeq.current || isCancelled(e)) return;
        setStatus("error");
        say(describeError(e));
      }
    }, mode === "word" ? debounceWordMs : debounceFtsMs);
    return () => clearTimeout(timer);
  }, [query, mode, api, debounceWordMs, debounceFtsMs, say]);

  /** صفحهٔ بعدی نتایج متن‌کامل */
  const loadMore = useCallback(async () => {
    if (mode !== "fulltext" || !hasMore || loadingMore.current) return;
    const seq = searchSeq.current;
    loadingMore.current = true;
    try {
      const page = await api.searchFulltext(query.trim(), results.length);
      if (seq !== searchSeq.current) return;
      setResults((r) => [...r, ...page.hits.map(ftsItem)]);
      setHasMore(page.hasMore);
    } catch (e) {
      if (!isCancelled(e)) say(describeError(e));
    } finally {
      loadingMore.current = false;
    }
  }, [api, mode, hasMore, query, results.length, say]);

  // ---------------- فهرست کناری ----------------
  const items: ListItem[] = useMemo(() => {
    if (tab === "history") return history.map((h) => ({ key: `h${h.wordNorm}`, word: h.word, wordNorm: h.wordNorm }));
    if (tab === "favorites") return favorites.map((f) => ({ key: `v${f.wordNorm}`, word: f.word, wordNorm: f.wordNorm }));
    return results;
  }, [tab, results, history, favorites]);
  const active = items.length ? Math.min(activeIndex, items.length - 1) : -1;

  const setTab = useCallback((t: Tab) => {
    setTabState(t);
    setActiveIndex(0);
  }, []);

  const changeQuery = useCallback((value: string) => {
    setQuery(value);
    setTabState("results");
  }, []);

  const clearQuery = useCallback(() => changeQuery(""), [changeQuery]);

  const moveActive = useCallback(
    (delta: number) => setActiveIndex((i) => moveIndex(Math.min(i, items.length - 1), delta, items.length)),
    [items.length],
  );

  // ---------------- باز کردن مدخل ----------------
  const open = useCallback(
    async (item: { wordNorm: string; word: string; focusId?: number }, opts: { offset?: number; record?: boolean } = {}) => {
      const { offset = 0, record = true } = opts;
      const seq = ++openSeq.current;
      setGroupLoading(true);
      try {
        const g = await api.getEntryGroup(item.wordNorm, { focusId: item.focusId, offset });
        if (seq !== openSeq.current) return;
        if (!g) {
          say("مدخلی با این نوشتار پیدا نشد.");
          return;
        }
        setGroup(g);
        setFocusEntryId(item.focusId ?? null);
        if (record) {
          try {
            const h = await api.recordVisit({ wordNorm: g.wordNorm, word: g.word });
            if (seq === openSeq.current) setHistory(h);
          } catch {
            say("ذخیرهٔ تاریخچه ممکن نشد.");
          }
        }
      } catch (e) {
        if (seq === openSeq.current) say(describeError(e));
      } finally {
        if (seq === openSeq.current) setGroupLoading(false);
      }
    },
    [api, say],
  );

  openRef.current = (item) => open(item);

  const openActive = useCallback(() => {
    if (tab === "results" && status === "loading" && query.trim()) {
      pendingOpen.current = true; // «تایپ و Enter» پشت سر هم: منتظر نتایج می‌مانیم
      return;
    }
    const it = items[active];
    if (it) void open(it);
  }, [items, active, open, tab, status, query]);

  const pick = useCallback(
    (index: number) => {
      const it = items[index];
      if (!it) return;
      setActiveIndex(index);
      void open(it);
    },
    [items, open],
  );

  /** صفحهٔ دیگری از یک گروه هم‌املای بزرگ (در تاریخچه ثبت نمی‌شود) */
  const gotoGroupOffset = useCallback(
    (offset: number) => {
      if (group) void open({ wordNorm: group.wordNorm, word: group.word }, { offset, record: false });
    },
    [group, open],
  );

  // ---------------- علاقه‌مندی‌ها و تاریخچه ----------------
  const isFavorite = !!group && favorites.some((f) => f.wordNorm === group.wordNorm);

  const toggleFavorite = useCallback(async () => {
    if (!group) return;
    try {
      const r = await api.toggleFavorite({ wordNorm: group.wordNorm, word: group.word } satisfies WordRef);
      setFavorites(r.favorites);
      say(r.isFavorite ? "به علاقه‌مندی‌ها افزوده شد." : "از علاقه‌مندی‌ها برداشته شد.");
    } catch {
      say("ذخیرهٔ علاقه‌مندی ممکن نشد.");
    }
  }, [api, group, say]);

  const clearHistory = useCallback(async () => {
    try {
      setHistory(await api.clearHistory());
    } catch {
      say("پاک کردن تاریخچه ممکن نشد.");
    }
  }, [api, say]);

  /** فهرست‌های تازهٔ این رایانه پس از «ورودی گرفتن» از پوشهٔ همگام‌سازی */
  const applySynced = useCallback((lists: { history: HistoryItem[]; favorites: FavoriteItem[] }) => {
    setHistory(lists.history);
    setFavorites(lists.favorites);
  }, []);

  // ---------------- تنظیمات ----------------
  const updateSettings = useCallback(
    async (patch: Partial<Settings>) => {
      const next: Settings = { ...settingsRef.current, ...patch };
      next.fontScale = clampScale(next.fontScale);
      settingsRef.current = next;
      setSettings(next);
      try {
        const saved = await api.saveSettings(next);
        settingsRef.current = saved;
        setSettings(saved);
      } catch {
        say("ذخیرهٔ تنظیمات ممکن نشد.");
      }
    },
    [api, say],
  );

  /** dir: ‎+1 بزرگ‌تر، ‎-1 کوچک‌تر، ‎0 بازنشانی */
  const bumpFont = useCallback(
    (dir: 1 | -1 | 0) => updateSettings({ fontScale: dir === 0 ? 1 : settingsRef.current.fontScale + dir * FONT_STEP }),
    [updateSettings],
  );

  const setMode = useCallback(
    (m: Mode) => {
      if (m === "fulltext" && !ftsAvailable) {
        say("این پایگاه بدون جست‌وجوی متن‌کامل ساخته شده است (build_db.py بدون --no-fts).");
        return;
      }
      setModeState(m);
      setTabState("results");
      void updateSettings({ fulltext: m === "fulltext" });
    },
    [ftsAvailable, say, updateSettings],
  );

  // تم: صفت data-theme روی <html> (system = بدون صفت) و هماهنگی نوار عنوان پنجره
  useEffect(() => {
    const root = document.documentElement;
    if (settings.theme === "system") root.removeAttribute("data-theme");
    else root.setAttribute("data-theme", settings.theme);
    api.setWindowTheme(settings.theme).catch(() => undefined);
  }, [settings.theme, api]);

  return {
    // وضعیت
    query, mode, ftsAvailable, status, tab, items, active, hasMore, group, focusEntryId, groupLoading,
    history, favorites, isFavorite, settings, notice,
    // کنش‌ها
    changeQuery, clearQuery, setMode, setTab, moveActive, openActive, pick, loadMore, gotoGroupOffset,
    toggleFavorite, clearHistory, applySynced, updateSettings, bumpFont, dismissNotice, say,
  };
}

export type Dictionary = ReturnType<typeof useDictionary>;
