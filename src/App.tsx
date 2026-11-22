import { useCallback, useEffect, useId, useRef, useState, type CSSProperties, type KeyboardEvent } from "react";
import type { Api } from "./api/api";
import { AboutDialog } from "./components/AboutDialog";
import { EntryView } from "./components/EntryView";
import { ErrorScreen } from "./components/ErrorScreen";
import { SearchBar } from "./components/SearchBar";
import { SettingsMenu } from "./components/SettingsMenu";
import { Sidebar } from "./components/Sidebar";
import { SyncDialog } from "./components/SyncDialog";
import { Toast } from "./components/Toast";
import { optionId } from "./components/VirtualList";
import { useDictionary, type DictionaryOptions } from "./hooks/useDictionary";
import { copyText } from "./lib/clipboard";
import { groupToPlainText } from "./lib/plainText";
import type { Bootstrap } from "./types";

interface Props {
  api: Api;
  boot: Bootstrap;
  /** فقط برای تست: زمان debounce */
  options?: DictionaryOptions;
}

export default function App({ api, boot, options }: Props) {
  const d = useDictionary(api, boot, options);
  const inputRef = useRef<HTMLInputElement>(null);
  const listId = useId();
  const [aboutOpen, setAboutOpen] = useState(false);
  const [syncOpen, setSyncOpen] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);

  // آخرین مقدارها برای شنوندهٔ سراسری کیبورد (یک‌بار ثبت می‌شود)
  const latest = useRef({ d, aboutOpen, syncOpen, menuOpen });
  latest.current = { d, aboutOpen, syncOpen, menuOpen };

  const focusSearch = useCallback((select: boolean) => {
    const el = inputRef.current;
    if (!el) return;
    el.focus();
    if (select) el.select();
  }, []);

  useEffect(() => {
    // کد فیزیکی کلید (e.code) اولویت دارد تا با صفحه‌کلید فارسی هم کار کند؛ نویسهٔ کلید (e.key) پشتیبان است
    const is = (e: globalThis.KeyboardEvent, codes: string[], keys: string[]) => codes.includes(e.code) || keys.includes(e.key);
    const onKey = (e: globalThis.KeyboardEvent) => {
      const { d, aboutOpen, syncOpen, menuOpen } = latest.current;
      const mod = e.ctrlKey || e.metaKey;
      if (mod && is(e, ["KeyK"], ["k", "K"])) {
        e.preventDefault();
        focusSearch(true);
      } else if (mod && is(e, ["Equal", "NumpadAdd"], ["=", "+"])) {
        e.preventDefault();
        d.bumpFont(1);
      } else if (mod && is(e, ["Minus", "NumpadSubtract"], ["-", "_"])) {
        e.preventDefault();
        d.bumpFont(-1);
      } else if (mod && is(e, ["Digit0", "Numpad0"], ["0"])) {
        e.preventDefault();
        d.bumpFont(0);
      } else if (e.key === "Escape") {
        if (aboutOpen) setAboutOpen(false);
        else if (syncOpen) setSyncOpen(false);
        else if (menuOpen) setMenuOpen(false);
        else if (document.activeElement !== inputRef.current) focusSearch(false);
        else if (d.query) d.clearQuery();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [focusSearch]);

  const onSearchKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.nativeEvent.isComposing) return;
    switch (e.key) {
      case "ArrowDown":
        e.preventDefault();
        d.moveActive(1);
        break;
      case "ArrowUp":
        e.preventDefault();
        d.moveActive(-1);
        break;
      case "PageDown":
        e.preventDefault();
        d.moveActive(8);
        break;
      case "PageUp":
        e.preventDefault();
        d.moveActive(-8);
        break;
      case "Enter":
        e.preventDefault();
        d.openActive();
        break;
    }
  };

  const onCopy = async () => {
    if (!d.group) return;
    d.say((await copyText(groupToPlainText(d.group))) ? "متن کپی شد." : "کپی ممکن نشد.");
  };

  if (boot.dbError) return <ErrorScreen message={boot.dbError} />;

  return (
    <div className="app" style={{ "--reading-scale": d.settings.fontScale } as CSSProperties}>
      <header className="topbar">
        <div className="brand">لغت‌نامهٔ دهخدا</div>
        <SearchBar
          value={d.query}
          onChange={d.changeQuery}
          onClear={() => {
            d.clearQuery();
            focusSearch(false);
          }}
          mode={d.mode}
          onModeChange={d.setMode}
          ftsAvailable={d.ftsAvailable}
          inputRef={inputRef}
          listId={listId}
          activeOptionId={d.active >= 0 ? optionId(listId, d.active) : undefined}
          loading={d.status === "loading"}
          onKeyDown={onSearchKeyDown}
        />
        <div className="tools">
          <SettingsMenu
            settings={d.settings}
            open={menuOpen}
            onOpenChange={setMenuOpen}
            onTheme={(theme) => void d.updateSettings({ theme })}
            onFont={d.bumpFont}
          />
          <button type="button" className="tool-btn" onClick={() => setSyncOpen(true)}>
            همگام‌سازی
          </button>
          <button type="button" className="tool-btn" onClick={() => setAboutOpen(true)}>
            درباره
          </button>
        </div>
      </header>
      <div className="body">
        <Sidebar
          listId={listId}
          tab={d.tab}
          onTab={d.setTab}
          historyCount={d.history.length}
          favoritesCount={d.favorites.length}
          mode={d.mode}
          status={d.status}
          hasMore={d.hasMore}
          items={d.items}
          active={d.active}
          onPick={d.pick}
          onNearEnd={d.loadMore}
          onClearHistory={() => void d.clearHistory()}
        />
        <main className="content" id="content">
          <EntryView
            group={d.group}
            loading={d.groupLoading}
            focusEntryId={d.focusEntryId}
            isFavorite={d.isFavorite}
            onToggleFavorite={() => void d.toggleFavorite()}
            onCopy={() => void onCopy()}
            onGoto={d.gotoGroupOffset}
            info={boot.info}
          />
        </main>
      </div>
      <Toast notice={d.notice} onDismiss={d.dismissNotice} />
      <AboutDialog open={aboutOpen} onClose={() => setAboutOpen(false)} info={boot.info} />
      <SyncDialog open={syncOpen} onClose={() => setSyncOpen(false)} api={api} onImported={d.applySynced} />
    </div>
  );
}
